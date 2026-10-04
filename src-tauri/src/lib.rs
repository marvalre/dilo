pub mod commands;
pub mod coordinator;
pub mod engine;
pub mod hotkey;
pub mod mascot;
pub mod models;
pub mod paste;
pub mod permissions;
pub mod recorder;
pub mod rules;
pub mod settings;
pub mod stats;
pub mod store;
pub mod tray;
pub mod updater;

use coordinator::Core;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const PANEL: &str = "panel";
/// Passed by the login item so Dilo starts quietly in the menu bar.
const HIDDEN_FLAG: &str = "--hidden";

/// Opens (or focuses) the panel. The window is destroyed on close to free RAM.
pub fn open_panel(app: &AppHandle, tab: Option<&str>) {
    let app2 = app.clone();
    let tab = tab.map(str::to_string);
    let _ = app.run_on_main_thread(move || {
        let win = match app2.get_webview_window(PANEL) {
            Some(w) => w,
            None => {
                let url = match &tab {
                    Some(t) => format!("index.html#{t}"),
                    None => "index.html".into(),
                };
                match WebviewWindowBuilder::new(&app2, PANEL, WebviewUrl::App(url.into()))
                    .title("Dilo")
                    .inner_size(920.0, 640.0)
                    .min_inner_size(760.0, 520.0)
                    .center()
                    .build()
                {
                    Ok(w) => {
                        // Show in the Dock while the window is open; back to menu-bar only when closed.
                        #[cfg(target_os = "macos")]
                        {
                            let _ = app2.set_activation_policy(tauri::ActivationPolicy::Regular);
                            let h = app2.clone();
                            w.on_window_event(move |e| {
                                if let tauri::WindowEvent::Destroyed = e {
                                    let _ = h.set_activation_policy(tauri::ActivationPolicy::Accessory);
                                }
                            });
                        }
                        w
                    }
                    Err(e) => return log::error!("panel: {e}"),
                }
            }
        };
        if let Some(t) = &tab {
            let _ = win.emit("panel://tab", t);
        }
        let _ = win.show();
        let _ = win.set_focus();
    });
}

/// The app used to be called Dicta (`com.dicta.app`). Move its data — history, settings, the
/// downloaded model — to the new folder and drop the old login item, so nothing is lost.
fn migrate_from_dicta(new_dir: &std::path::Path) {
    let Some(parent) = new_dir.parent() else { return };
    let old_dir = parent.join("com.dicta.app");
    if new_dir.exists() || !old_dir.exists() {
        return;
    }
    if std::fs::rename(&old_dir, new_dir).is_err() {
        return;
    }
    // SQLite sidecar files (WAL/SHM) must follow the database or recent rows are lost.
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::rename(new_dir.join(format!("dicta.db{suffix}")), new_dir.join(format!("dilo.db{suffix}")));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let _ = std::fs::remove_file(std::path::Path::new(&home).join("Library/LaunchAgents/Dicta.plist"));
    }
    log::info!("migrated data from {old_dir:?}");
}

/// False when running from a disk image or macOS's temporary "translocated" copy.
pub(crate) fn is_stable_location() -> bool {
    std::env::current_exe()
        .map(|p| {
            let p = p.to_string_lossy();
            !p.starts_with("/Volumes/") && !p.contains("/AppTranslocation/")
        })
        .unwrap_or(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,ort=warn,enigo=warn,transcribe_rs=warn")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| open_panel(app, None)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![HIDDEN_FLAG])))
        .invoke_handler(tauri::generate_handler![
            commands::get_history,
            commands::update_dictation,
            commands::clear_history,
            commands::storage_info,
            commands::capture_hotkey,
            commands::list_rules,
            commands::save_rule,
            commands::delete_rule,
            commands::delete_dictation,
            commands::get_stats,
            commands::get_settings,
            commands::save_settings,
            commands::list_microphones,
            commands::model_status,
            commands::download_model,
            commands::copy_text,
            commands::permissions,
            commands::open_accessibility_settings,
            commands::open_microphone_settings,
            commands::check_update,
            commands::install_update,
            commands::pending_update,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            migrate_from_dicta(&data_dir);
            let core = Core::new(data_dir)?;
            app.manage(core.clone());

            // Re-point the login item at this copy of the app (it may have moved).
            if core.settings.read().unwrap().launch_at_login && is_stable_location() {
                use tauri_plugin_autostart::ManagerExt;
                if let Err(e) = app.autolaunch().enable() {
                    log::error!("autostart: {e}");
                }
            }

            app.manage(updater::UpdateState::default());
            tauri::async_runtime::spawn(updater::background(app.handle().clone()));

            mascot::create(app.handle())?;
            tray::create(app.handle())?;
            coordinator::spawn_idle_unloader(core.clone());

            if !hotkey::has_accessibility() {
                hotkey::prompt_accessibility();
            }
            let handle = app.handle().clone();
            let hotkey = core.settings.read().unwrap().hotkey.clone();
            match hotkey::spawn(&hotkey, move |pressed| coordinator::on_hotkey(&handle, pressed)) {
                Ok(h) => *core.hotkey.lock().unwrap() = Some(h),
                Err(e) => log::error!("hotkey: {e:#}"),
            }

            if let Ok(wav) = std::env::var("DILO_SIMULATE_WAV") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    if let Err(e) = coordinator::simulate(&handle, std::path::Path::new(&wav)) {
                        log::error!("simulate: {e:#}");
                    }
                });
            }

            let needs_setup = !models::is_ready(&core.model_dir())
                || !hotkey::has_accessibility()
                || !permissions::microphone_granted();
            let hidden = std::env::args().any(|a| a == HIDDEN_FLAG);
            if needs_setup {
                open_panel(app.handle(), Some("settings"));
            } else if !hidden {
                open_panel(app.handle(), Some("home"));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Dilo")
        .run(|app, event| match event {
            // Keep running in the tray when the window closes.
            tauri::RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            // Opening Dilo again (Finder, Spotlight, Dock) shows the window.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => open_panel(app, None),
            _ => {}
        });
}
