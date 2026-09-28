//! Global push-to-talk key via handy-keys. Runs on its own thread and reports
//! press/release. The key can be changed at runtime with `HotkeyHandle::set`.

use anyhow::Result;
use handy_keys::{Hotkey, HotkeyId, HotkeyManager, HotkeyState};
use std::sync::mpsc;
use std::time::Duration;

pub struct HotkeyHandle {
    tx: mpsc::Sender<String>,
}

impl HotkeyHandle {
    /// Replace the active hotkey. Invalid strings are logged and ignored.
    pub fn set(&self, hotkey: &str) {
        let _ = self.tx.send(hotkey.to_string());
    }
}

pub fn validate(hotkey: &str) -> Result<(), String> {
    hotkey.parse::<Hotkey>().map(|_| ()).map_err(|e| format!("{e}"))
}

pub fn spawn(hotkey: &str, on_event: impl Fn(bool) + Send + 'static) -> Result<HotkeyHandle> {
    let (tx, rx) = mpsc::channel::<String>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let initial = hotkey.to_string();

    std::thread::Builder::new().name("hotkey".into()).spawn(move || {
        let manager = match HotkeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("hotkey manager: {e}")));
                return;
            }
        };
        let mut current: Option<HotkeyId> = register(&manager, &initial);
        let _ = ready_tx.send(Ok(()));
        let mut held = false;
        loop {
            if let Ok(next) = rx.try_recv() {
                if let Some(id) = current.take() {
                    let _ = manager.unregister(id);
                }
                current = register(&manager, &next);
                held = false;
            }
            while let Some(ev) = manager.try_recv() {
                let pressed = matches!(ev.state, HotkeyState::Pressed);
                // Key auto-repeat can send repeated presses; forward only edges.
                if pressed != held {
                    held = pressed;
                    on_event(pressed);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })?;

    ready_rx.recv()?.map_err(anyhow::Error::msg)?;
    Ok(HotkeyHandle { tx })
}

fn register(manager: &HotkeyManager, hotkey: &str) -> Option<HotkeyId> {
    match hotkey.parse::<Hotkey>() {
        Ok(h) => match manager.register(h) {
            Ok(id) => {
                log::info!("hotkey registered: {hotkey}");
                Some(id)
            }
            Err(e) => {
                log::error!("register {hotkey}: {e}");
                None
            }
        },
        Err(e) => {
            log::error!("parse hotkey {hotkey}: {e}");
            None
        }
    }
}

pub fn has_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        handy_keys::check_accessibility()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = handy_keys::open_accessibility_settings();
    }
}

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn default_and_preset_hotkeys_parse() {
        for k in ["Fn", "CtrlRight", "OptRight", "CmdRight", "Ctrl+Space"] {
            assert!(validate(k).is_ok(), "{k}: {:?}", validate(k));
        }
        assert!(validate("NotAKey+++").is_err());
    }
}
