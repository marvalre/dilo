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

/// Records the next key or combination the user presses and returns it in
/// handy-keys syntax ("Fn", "CtrlRight", "Cmd+Shift+D"). None on Escape/timeout.
pub fn capture(timeout: Duration) -> Result<Option<String>> {
    use handy_keys::{Key, KeyboardListener, Modifiers};
    let listener = KeyboardListener::new().map_err(|e| anyhow::anyhow!("{e}"))?;
    let deadline = std::time::Instant::now() + timeout;
    // Largest set of modifiers held so far, for modifier-only hotkeys like "Fn".
    let mut best = Modifiers::empty();

    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        let Ok(ev) = listener.recv_timeout(left) else { return Ok(None) };
        match ev.key {
            Some(Key::Escape) if ev.is_key_down => return Ok(None),
            Some(key) if ev.is_key_down => {
                let mods = generic_sides(ev.modifiers);
                let hotkey = Hotkey::new(mods, Some(key)).map_err(|e| anyhow::anyhow!("{e}"))?;
                return Ok(Some(format_hotkey(&hotkey)));
            }
            _ => {}
        }
        if ev.modifiers.bits().count_ones() > best.bits().count_ones() {
            best = ev.modifiers;
        }
        if ev.modifiers.is_empty() && !best.is_empty() {
            let hotkey = Hotkey::new(best, None).map_err(|e| anyhow::anyhow!("{e}"))?;
            return Ok(Some(format_hotkey(&hotkey)));
        }
    }
}

/// With a regular key, either Cmd (left or right) should work: drop the side.
fn generic_sides(m: handy_keys::Modifiers) -> handy_keys::Modifiers {
    use handy_keys::Modifiers as M;
    let mut out = m & M::FN;
    for (left, right, both) in [
        (M::CMD_LEFT, M::CMD_RIGHT, M::CMD),
        (M::SHIFT_LEFT, M::SHIFT_RIGHT, M::SHIFT),
        (M::CTRL_LEFT, M::CTRL_RIGHT, M::CTRL),
        (M::OPT_LEFT, M::OPT_RIGHT, M::OPT),
    ] {
        if m.intersects(left | right) {
            out |= both;
        }
    }
    out
}

/// "Cmd+Shift+D" style (the Modifiers Display plus the key), parseable by Hotkey::from_str.
fn format_hotkey(h: &Hotkey) -> String {
    let mods = h.modifiers.to_string();
    match (&h.key, mods.is_empty()) {
        (Some(k), true) => k.to_string(),
        (Some(k), false) => format!("{mods}+{k}"),
        (None, _) => mods,
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
    fn formatted_hotkeys_parse_back() {
        use handy_keys::{Hotkey, Key, Modifiers};
        let cases = [
            Hotkey::new(Modifiers::FN, None).unwrap(),
            Hotkey::new(Modifiers::CTRL_RIGHT, None).unwrap(),
            Hotkey::new(super::generic_sides(Modifiers::CMD_LEFT | Modifiers::SHIFT_RIGHT), Some(Key::D)).unwrap(),
        ];
        for h in cases {
            let s = super::format_hotkey(&h);
            let back: Hotkey = s.parse().unwrap();
            assert_eq!(back, h, "{s}");
        }
        assert_eq!(super::format_hotkey(&"Cmd+Shift+D".parse().unwrap()), "Shift+Cmd+D");
    }

    #[test]
    fn default_and_preset_hotkeys_parse() {
        for k in ["Fn", "CtrlRight", "OptRight", "CmdRight", "Ctrl+Space"] {
            assert!(validate(k).is_ok(), "{k}: {:?}", validate(k));
        }
        assert!(validate("NotAKey+++").is_err());
    }
}
