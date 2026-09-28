//! Tauri commands used by the panel (see src/panel/api.ts for the TS side).

use crate::coordinator::{Core, ModelStatus};
use crate::{hotkey, models, settings::Settings, stats::Stats, store::Dictation};
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

type Core_<'a> = State<'a, Arc<Core>>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub fn get_history(core: Core_, query: String, offset: u32) -> Result<Vec<Dictation>, String> {
    let q = if query.trim().is_empty() { None } else { Some(query.as_str()) };
    core.store.list(q, 50, offset).map_err(err)
}

#[tauri::command]
pub fn delete_dictation(app: AppHandle, core: Core_, id: i64) -> Result<(), String> {
    core.store.delete(id).map_err(err)?;
    let _ = app.emit("history://changed", ());
    Ok(())
}

#[tauri::command]
pub fn get_stats(core: Core_) -> Result<Stats, String> {
    core.store.stats(crate::coordinator::now_ms()).map_err(err)
}

#[tauri::command]
pub fn get_settings(core: Core_) -> Settings {
    core.settings.read().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, core: Core_, settings: Settings) -> Result<(), String> {
    hotkey::validate(&settings.hotkey).map_err(|e| format!("Tecla no válida: {e}"))?;
    let old_hotkey = core.settings.read().unwrap().hotkey.clone();
    settings.save(&core.data_dir).map_err(err)?;
    if settings.hotkey != old_hotkey {
        if let Some(h) = core.hotkey.lock().unwrap().as_ref() {
            h.set(&settings.hotkey);
        }
    }
    *core.settings.write().unwrap() = settings;
    crate::tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub fn model_status(core: Core_) -> ModelStatus {
    core.model_status()
}

#[tauri::command]
pub fn download_model(app: AppHandle, core: Core_) -> Result<(), String> {
    {
        let mut m = core.model.lock().unwrap();
        if m.downloading {
            return Ok(());
        }
        m.downloading = true;
        m.error = None;
    }
    let core = core.inner().clone();
    std::thread::spawn(move || {
        let dir = core.model_dir();
        let result = models::download(&dir, |done, total| {
            {
                let mut m = core.model.lock().unwrap();
                m.done = done;
                m.total = total;
            }
            let _ = app.emit("model://status", core.model_status());
        });
        {
            let mut m = core.model.lock().unwrap();
            m.downloading = false;
            m.error = result.err().map(|e| format!("{e:#}"));
        }
        let _ = app.emit("model://status", core.model_status());
    });
    Ok(())
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).map_err(err)
}

#[derive(serde::Serialize)]
pub struct Permissions {
    accessibility: bool,
    microphone: bool,
}

#[tauri::command]
pub fn permissions() -> Permissions {
    Permissions { accessibility: hotkey::has_accessibility(), microphone: crate::permissions::microphone_granted() }
}

#[tauri::command]
pub fn open_accessibility_settings() {
    hotkey::open_accessibility_settings();
}

#[tauri::command]
pub fn open_microphone_settings() {
    crate::permissions::request_or_open_microphone();
}
