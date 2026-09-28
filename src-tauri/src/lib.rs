pub mod commands;
pub mod coordinator;
pub mod engine;
pub mod hotkey;
pub mod mascot;
pub mod models;
pub mod paste;
pub mod permissions;
pub mod recorder;
pub mod settings;
pub mod stats;
pub mod store;
pub mod tray;

use coordinator::Core;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const PANEL: &str = "panel";

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
                    .title("Dicta")
                    .inner_size(440.0, 620.0)
                    .min_inner_size(380.0, 480.0)
                    .center()
                    .build()
                {
                    Ok(w) => w,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,ort=warn,enigo=warn,transcribe_rs=warn")).init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_history,
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
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let core = Core::new(data_dir)?;
            app.manage(core.clone());

            mascot::create(app.handle())?;
            tray::create(app.handle())?;
            coordinator::spawn_idle_unloader(core.clone());

            let handle = app.handle().clone();
            let hotkey = core.settings.read().unwrap().hotkey.clone();
            match hotkey::spawn(&hotkey, move |pressed| coordinator::on_hotkey(&handle, pressed)) {
                Ok(h) => *core.hotkey.lock().unwrap() = Some(h),
                Err(e) => log::error!("hotkey: {e:#}"),
            }

            if let Ok(wav) = std::env::var("DICTA_SIMULATE_WAV") {
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
            if needs_setup {
                open_panel(app.handle(), Some("settings"));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Dicta")
        .run(|_app, event| {
            // Keep running in the tray when the panel closes.
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
