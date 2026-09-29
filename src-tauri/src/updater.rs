//! In-app updates. Dilo looks for a newer release on GitHub, checks its signature against the public key
//! in tauri.conf.json, downloads it and swaps the app in place — no new Gatekeeper prompt, and the
//! macOS permissions survive because the new build is signed with the same certificate.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: Option<String>,
}

#[derive(Serialize, Clone)]
struct Progress {
    done: u64,
    total: u64,
}

#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
    available: Mutex<Option<UpdateInfo>>,
    installing: Mutex<bool>,
}

impl UpdateState {
    pub fn available(&self) -> Option<UpdateInfo> {
        self.available.lock().unwrap().clone()
    }
}

fn friendly(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    let l = raw.to_lowercase();
    if l.contains("network") || l.contains("dns") || l.contains("connect") || l.contains("timed out") || l.contains("error sending request") {
        "No se pudo conectar. Revisa tu internet e inténtalo de nuevo.".into()
    } else if l.contains("signature") {
        "La firma de la actualización no es válida; no se instaló nada.".into()
    } else if l.contains("could not fetch a valid release") || l.contains("404") {
        "Todavía no hay información de versiones disponible. Inténtalo más tarde.".into()
    } else {
        raw
    }
}

/// Asks GitHub whether a newer version exists. Remembers it so `install` can use it.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    if !crate::is_stable_location() {
        return Err("Mueve Dilo a la carpeta Aplicaciones para poder actualizarla desde la app.".into());
    }
    let state = app.state::<UpdateState>();
    let update = app.updater().map_err(friendly)?.check().await.map_err(friendly)?;
    let info = update.as_ref().map(|u| UpdateInfo { version: u.version.clone(), current: u.current_version.clone(), notes: u.body.clone() });
    *state.pending.lock().unwrap() = update;
    *state.available.lock().unwrap() = info.clone();
    // Menus can only be rebuilt on the main thread; doing it from here crashes macOS.
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || crate::tray::refresh(&handle));
    Ok(info)
}

/// Downloads, verifies and installs the update found by `check`, then relaunches.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<UpdateState>();
    {
        let mut busy = state.installing.lock().unwrap();
        if *busy {
            return Ok(());
        }
        *busy = true;
    }
    let result = async {
        let update = state.pending.lock().unwrap().take().ok_or("Primero busca actualizaciones.")?;
        let mut done = 0u64;
        update
            .download_and_install(
                |chunk, total| {
                    done += chunk as u64;
                    let _ = app.emit("update://progress", Progress { done, total: total.unwrap_or(0) });
                },
                || {},
            )
            .await
            .map_err(friendly)
    }
    .await;
    *state.installing.lock().unwrap() = false;
    result?;
    app.restart()
}

/// Quiet check shortly after launch and every few hours; tells the window and the menu when there is news.
pub async fn background(app: AppHandle) {
    tokio_sleep(Duration::from_secs(20)).await;
    loop {
        let enabled = app.try_state::<std::sync::Arc<crate::coordinator::Core>>().map(|c| c.settings.read().unwrap().auto_update).unwrap_or(false);
        if enabled {
            let known = app.state::<UpdateState>().available();
            match check(&app).await {
                Ok(Some(info)) if known.as_ref() != Some(&info) => {
                    log::info!("update available: {}", info.version);
                    let _ = app.emit("update://available", &info);
                }
                Ok(_) => {}
                Err(e) => log::info!("update check: {e}"),
            }
        }
        tokio_sleep(Duration::from_secs(6 * 3600)).await;
    }
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}
