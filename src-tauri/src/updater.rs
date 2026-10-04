//! In-app updates. Dilo looks for a newer release on GitHub, checks its signature against the public key
//! in tauri.conf.json, downloads it and swaps the app in place — no new Gatekeeper prompt, and the
//! macOS permissions survive because the new build is signed with the same certificate.

use serde::Serialize;
use std::sync::{Mutex, MutexGuard, PoisonError};
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

// A panic while a lock was held must not make every later update call panic.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Clears the "installing" flag however `install` ends (error, early return, panic, cancelled future).
struct BusyGuard<'a>(&'a Mutex<bool>);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        *lock(self.0) = false;
    }
}

impl UpdateState {
    pub fn available(&self) -> Option<UpdateInfo> {
        lock(&self.available).clone()
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
    *lock(&state.pending) = update;
    *lock(&state.available) = info.clone();
    // Menus can only be rebuilt on the main thread; doing it from here crashes macOS.
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || crate::tray::refresh(&handle));
    Ok(info)
}

/// Downloads, verifies and installs the update found by `check`, then relaunches.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<UpdateState>();
    {
        let mut busy = lock(&state.installing);
        if *busy {
            return Ok(());
        }
        *busy = true;
    }
    let _busy = BusyGuard(&state.installing);
    let pending = lock(&state.pending).take();
    let update = match pending {
        Some(u) => u,
        // A previous attempt consumed it (failed download) or nothing was checked yet: look again.
        None => {
            if !crate::is_stable_location() {
                return Err("Mueve Dilo a la carpeta Aplicaciones para poder actualizarla desde la app.".into());
            }
            app.updater()
                .map_err(friendly)?
                .check()
                .await
                .map_err(friendly)?
                .ok_or_else(|| "Ya tienes la última versión.".to_string())?
        }
    };
    let mut done = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                done = done.saturating_add(chunk as u64);
                let _ = app.emit("update://progress", Progress { done, total: total.unwrap_or(0) });
            },
            || {},
        )
        .await
        .map_err(friendly)?;
    app.restart()
}

/// Quiet check shortly after launch and every few hours; tells the window and the menu when there is news.
pub async fn background(app: AppHandle) {
    tokio_sleep(Duration::from_secs(20)).await;
    loop {
        let enabled = app.try_state::<std::sync::Arc<crate::coordinator::Core>>().map(|c| c.settings.read().unwrap_or_else(PoisonError::into_inner).auto_update).unwrap_or(false);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_maps_known_errors() {
        assert!(friendly("error sending request for url").starts_with("No se pudo conectar"));
        assert!(friendly("Signature verification failed").contains("firma"));
        assert!(friendly("Could not fetch a valid release JSON").contains("versiones"));
        assert_eq!(friendly("weird"), "weird");
    }

    #[test]
    fn busy_guard_resets_flag_even_when_poisoned() {
        let flag = Mutex::new(true);
        drop(BusyGuard(&flag));
        assert!(!*lock(&flag));
        let m = std::sync::Arc::new(Mutex::new(true));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        drop(BusyGuard(&m));
        assert!(!*lock(&m));
    }
}
