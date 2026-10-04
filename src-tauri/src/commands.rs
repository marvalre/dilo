//! Tauri commands used by the panel (see src/panel/api.ts for the TS side).

use crate::coordinator::{Core, ModelStatus};
use crate::rules::{Rule, RuleKind};
use crate::{hotkey, models, settings::Settings, stats::Stats, store::{self, Dictation}};
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

type Core_<'a> = State<'a, Arc<Core>>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// A panic elsewhere while a lock was held must not turn every later command into a panic.
fn read<T>(l: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    l.read().unwrap_or_else(PoisonError::into_inner)
}

fn write<T>(l: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    l.write().unwrap_or_else(PoisonError::into_inner)
}

fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
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
pub fn update_dictation(app: AppHandle, core: Core_, id: i64, text: String) -> Result<Dictation, String> {
    let d = core.store.update_text(id, &text).map_err(err)?.ok_or("Dictado no encontrado")?;
    let _ = app.emit("history://changed", ());
    Ok(d)
}

#[tauri::command]
pub fn clear_history(app: AppHandle, core: Core_, older_than_days: Option<u32>) -> Result<usize, String> {
    let before = older_than_days.map(|d| store::cutoff_ms(crate::coordinator::now_ms(), d));
    let n = core.store.clear(before).map_err(err)?;
    let _ = app.emit("history://changed", ());
    Ok(n)
}

#[derive(serde::Serialize)]
pub struct StorageInfo {
    model_bytes: u64,
    history_bytes: u64,
    total_bytes: u64,
    warn_bytes: u64,
    dictations: u64,
}

#[tauri::command]
pub fn storage_info(core: Core_) -> Result<StorageInfo, String> {
    let model_bytes = store::dir_size(&core.data_dir.join("models"));
    let history_bytes = core.store.size_bytes().map_err(err)?;
    let warn_bytes = read(&core.settings).storage_warn_bytes();
    Ok(StorageInfo {
        model_bytes,
        history_bytes,
        total_bytes: model_bytes.saturating_add(history_bytes),
        warn_bytes,
        dictations: core.store.count().map_err(err)?,
    })
}

#[tauri::command]
pub fn get_settings(core: Core_) -> Settings {
    read(&core.settings).clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, core: Core_, settings: Settings) -> Result<(), String> {
    hotkey::validate(&settings.hotkey).map_err(|e| format!("Tecla no válida: {e}"))?;
    update_settings(&app, &core, |s| *s = settings)
}

/// The one place settings change: holds the lock for the whole update so the
/// panel and the tray can't overwrite each other, then persists and notifies.
pub fn update_settings(app: &AppHandle, core: &Core, change: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let mut guard = write(&core.settings);
    let old = guard.clone();
    let mut next = old.clone();
    change(&mut next);
    let mut next = next.sanitized();
    if next.launch_at_login != old.launch_at_login {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let r = if next.launch_at_login { al.enable() } else { al.disable() };
        if let Err(e) = r {
            log::error!("autostart: {e}");
            next.launch_at_login = old.launch_at_login;
        }
    }
    if let Err(e) = next.save(&core.data_dir) {
        // Keep the OS login item in sync with what is actually stored.
        if next.launch_at_login != old.launch_at_login {
            use tauri_plugin_autostart::ManagerExt;
            let al = app.autolaunch();
            let _ = if old.launch_at_login { al.enable() } else { al.disable() };
        }
        return Err(err(e));
    }
    if next.hotkey != old.hotkey {
        if let Some(h) = lock(&core.hotkey).as_ref() {
            h.set(&next.hotkey);
        }
    }
    *guard = next.clone();
    drop(guard);
    let _ = app.emit("settings://changed", &next);
    crate::tray::refresh(app);
    Ok(())
}

/// Blocks until the user presses a key/combination (max 10 s). Dictation is paused meanwhile.
#[tauri::command]
pub async fn capture_hotkey(core: State<'_, Arc<Core>>) -> Result<Option<String>, String> {
    use std::sync::atomic::Ordering;
    if !hotkey::has_accessibility() {
        return Err("Primero dale permiso de Accesibilidad a Dilo (más abajo, en Permisos).".into());
    }
    let core = core.inner().clone();
    if core.capturing.swap(true, Ordering::SeqCst) {
        return Err("Ya estoy esperando una tecla".into());
    }
    let result = tauri::async_runtime::spawn_blocking(|| {
        let r = hotkey::capture(std::time::Duration::from_secs(10));
        // Let the keys come back up before dictation listens again.
        std::thread::sleep(std::time::Duration::from_millis(400));
        r
    })
    .await
    .map_err(err)
    .and_then(|r| r.map_err(err));
    core.capturing.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
pub fn list_rules(core: Core_) -> Result<Vec<Rule>, String> {
    core.store.rules().map_err(err)
}

#[derive(serde::Deserialize)]
pub struct NewRule {
    id: Option<i64>,
    kind: RuleKind,
    from: String,
    to: String,
    enabled: bool,
}

#[tauri::command]
pub fn save_rule(core: Core_, rule: NewRule) -> Result<Rule, String> {
    crate::rules::validate(&rule.from, &rule.to)?;
    core.store.save_rule(rule.id, rule.kind, &rule.from, &rule.to, rule.enabled).map_err(err)
}

#[tauri::command]
pub fn delete_rule(core: Core_, id: i64) -> Result<(), String> {
    core.store.delete_rule(id).map_err(err)
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
        let mut m = lock(&core.model);
        if m.downloading {
            return Ok(());
        }
        m.downloading = true;
        m.error = None;
    }
    let core = core.inner().clone();
    std::thread::spawn(move || {
        let dir = core.model_dir();
        // A panic inside the download must not leave `downloading` stuck on true forever.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            models::download(&dir, |done, total| {
                {
                    let mut m = lock(&core.model);
                    m.done = done;
                    m.total = total;
                }
                let _ = app.emit("model://status", core.model_status());
            })
        }))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("la descarga falló de forma inesperada")));
        {
            let mut m = lock(&core.model);
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
    hotkey::prompt_accessibility();
    hotkey::open_accessibility_settings();
}

#[tauri::command]
pub fn open_microphone_settings() {
    crate::permissions::request_or_open_microphone();
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<crate::updater::UpdateInfo>, String> {
    crate::updater::check(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    crate::updater::install(&app).await
}

#[tauri::command]
pub fn pending_update(state: State<'_, crate::updater::UpdateState>) -> Option<crate::updater::UpdateInfo> {
    state.available()
}
