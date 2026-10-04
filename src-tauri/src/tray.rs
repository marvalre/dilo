//! Menu-bar / system-tray icon and menu.

use crate::coordinator::Core;
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const TRAY_ID: &str = "dilo";
pub const LANGUAGES: [(&str, &str); 7] = [
    ("es", "Español"),
    ("en", "English"),
    ("auto", "Automático"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("pt", "Português"),
    ("it", "Italiano"),
];

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let core = app.state::<Arc<Core>>();
    let current = core.settings.read().unwrap().language.clone();
    let hotkey = core.settings.read().unwrap().hotkey.clone();

    let langs = LANGUAGES
        .iter()
        .map(|(code, name)| {
            CheckMenuItem::with_id(app, format!("lang:{code}"), *name, true, *code == current, None::<&str>)
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let lang_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> =
        langs.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
    let language = Submenu::with_items(app, "Idioma", true, &lang_refs)?;

    let hint = MenuItem::with_id(app, "hint", format!("Mantén {hotkey} y habla"), false, None::<&str>)?;
    let home = MenuItem::with_id(app, "open:home", "Abrir Dilo…", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "open:history", "Historial…", true, None::<&str>)?;
    let stats = MenuItem::with_id(app, "open:dictionary", "Diccionario…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "open:settings", "Ajustes…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Salir de Dilo", true, Some("CmdOrCtrl+Q"))?;
    let sep = || PredefinedMenuItem::separator(app);
    if let Some(u) = app.state::<crate::updater::UpdateState>().available() {
        let update = MenuItem::with_id(app, "open:settings", format!("Actualizar a Dilo {}…", u.version), true, None::<&str>)?;
        return Menu::with_items(app, &[&hint, &sep()?, &update, &home, &history, &stats, &settings, &language, &sep()?, &quit]);
    }
    Menu::with_items(app, &[&hint, &sep()?, &home, &history, &stats, &settings, &language, &sep()?, &quit])
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Dilo")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "quit" {
                app.exit(0);
            } else if let Some(tab) = id.strip_prefix("open:") {
                crate::open_panel(app, Some(tab));
            } else if let Some(code) = id.strip_prefix("lang:") {
                let core = app.state::<Arc<Core>>();
                let code = code.to_string();
                if let Err(e) = crate::commands::update_settings(app, &core, |s| s.language = code) {
                    log::error!("language: {e}");
                }
                // The OS already toggled the check mark on click; redraw from the real setting
                // (also covers clicking the current language, which would otherwise uncheck it).
                refresh(app);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = event {
                crate::open_panel(tray.app_handle(), None);
            }
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the menu so check marks and the hotkey hint stay current.
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}
