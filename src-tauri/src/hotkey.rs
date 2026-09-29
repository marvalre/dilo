//! Global push-to-talk key via handy-keys. Runs on its own thread and reports
//! press/release. The key can be changed at runtime with `HotkeyHandle::set`.

use anyhow::Result;
use handy_keys::{Hotkey, HotkeyId, HotkeyManager, HotkeyState, Modifiers};
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
        let _ = ready_tx.send(Ok(()));
        // Without Accessibility permission the manager can't start. Keep retrying so
        // the key starts working as soon as the user grants it — no restart needed.
        let mut warned = false;
        let manager = loop {
            match HotkeyManager::new() {
                Ok(m) => break m,
                Err(e) => {
                    if !warned {
                        log::warn!("hotkey manager: {e} (retrying until permission is granted)");
                        warned = true;
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        };
        let mut pending = None;
        while let Ok(next) = rx.try_recv() {
            pending = Some(next); // key changed while waiting for permission
        }
        let mut current: Option<HotkeyId> = None;
        let mut poll: Option<Modifiers> = None;
        let activate = |hotkey: &str, current: &mut Option<HotkeyId>, poll: &mut Option<Modifiers>| {
            if let Some(id) = current.take() {
                let _ = manager.unregister(id);
            }
            *poll = modifier_only(hotkey);
            if poll.is_some() {
                log::info!("hotkey registered: {hotkey} (physical state)");
            } else {
                *current = register(&manager, hotkey);
            }
        };
        activate(pending.as_deref().unwrap_or(&initial), &mut current, &mut poll);
        let mut held = false;
        loop {
            if let Ok(next) = rx.try_recv() {
                activate(&next, &mut current, &mut poll);
                if held {
                    // The old key is no longer watched, so its release will never arrive.
                    held = false;
                    on_event(false);
                }
            }
            while let Some(ev) = manager.try_recv() {
                if poll.is_some() {
                    continue;
                }
                let pressed = matches!(ev.state, HotkeyState::Pressed);
                // Key auto-repeat can send repeated presses; forward only edges.
                if pressed != held {
                    held = pressed;
                    on_event(pressed);
                }
            }
            // Modifier-only keys (Fn, Option…) are read from the keyboard's real
            // state: counting events drifts when other apps (or our own Cmd+V)
            // send modifier changes while the key is held.
            if let Some(mods) = poll {
                if let Some(state) = physical::state(mods) {
                    let next = if held { state.required } else { state.required && !state.others };
                    if next != held {
                        held = next;
                        on_event(next);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })?;

    ready_rx.recv()?.map_err(anyhow::Error::msg)?;
    Ok(HotkeyHandle { tx })
}

/// The modifiers of a hotkey that has no regular key, where we can poll the keyboard.
fn modifier_only(hotkey: &str) -> Option<Modifiers> {
    let h = hotkey.parse::<Hotkey>().ok()?;
    (h.key.is_none() && cfg!(target_os = "macos")).then_some(h.modifiers)
}

mod physical {
    use handy_keys::Modifiers as M;

    pub struct State {
        /// Every modifier of the hotkey is down.
        pub required: bool,
        /// Some other modifier is down too.
        pub others: bool,
    }

    // Device-dependent modifier bits (IOKit NX_DEVICE*KEYMASK) and the Fn flag.
    const GROUPS: [(M, M, u64, u64); 4] = [
        (M::CTRL_LEFT, M::CTRL_RIGHT, 0x0000_0001, 0x0000_2000),
        (M::SHIFT_LEFT, M::SHIFT_RIGHT, 0x0000_0002, 0x0000_0004),
        (M::CMD_LEFT, M::CMD_RIGHT, 0x0000_0008, 0x0000_0010),
        (M::OPT_LEFT, M::OPT_RIGHT, 0x0000_0020, 0x0000_0040),
    ];
    const FN: u64 = 0x0080_0000;

    pub fn evaluate(mods: M, flags: u64) -> State {
        let mut required = true;
        let mut others = false;
        for (left, right, lbit, rbit) in GROUPS {
            let (l, r) = (flags & lbit != 0, flags & rbit != 0);
            match (mods.contains(left), mods.contains(right)) {
                (true, true) => required &= l || r,
                (true, false) => required &= l,
                (false, true) => required &= r,
                (false, false) => others |= l || r,
            }
        }
        let fn_down = flags & FN != 0;
        if mods.contains(M::FN) {
            required &= fn_down;
        } else {
            others |= fn_down;
        }
        State { required, others }
    }

    #[cfg(target_os = "macos")]
    pub fn state(mods: M) -> Option<State> {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventSourceFlagsState(state_id: i32) -> u64;
        }
        const HID_SYSTEM_STATE: i32 = 1;
        let flags = unsafe { CGEventSourceFlagsState(HID_SYSTEM_STATE) };
        Some(evaluate(mods, flags))
    }

    #[cfg(not(target_os = "macos"))]
    pub fn state(_mods: M) -> Option<State> {
        None
    }
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
            // A bare letter, digit or space would start dictating on every keystroke.
            Some(key) if ev.is_key_down && ev.modifiers.is_empty() && is_typing_key(&key) => {}
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

fn is_typing_key(key: &handy_keys::Key) -> bool {
    let name = key.to_string();
    name == "Space" || name == "Return" || name == "Tab" || name == "Delete" || (name.chars().count() == 1 && name.chars().all(char::is_alphanumeric))
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

/// Shows macOS's own "Dilo would like to control this computer" prompt, which also
/// adds Dilo (with its current signature) to the Accessibility list.
pub fn prompt_accessibility() {
    #[cfg(target_os = "macos")]
    {
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
        }
        let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
        let opts = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
        unsafe {
            AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef());
        }
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

    #[test]
    fn physical_state_matches_sides_and_extras() {
        use handy_keys::Modifiers as M;
        let opt_left = 0x20 | 0x80000;
        let s = super::physical::evaluate(M::OPT_LEFT, opt_left);
        assert!(s.required && !s.others);
        let s = super::physical::evaluate(M::OPT_LEFT, 0x40);
        assert!(!s.required);
        let s = super::physical::evaluate(M::OPT_LEFT, opt_left | 0x08);
        assert!(s.required && s.others, "Cmd held too");
        let s = super::physical::evaluate(M::FN, 0x80_0000);
        assert!(s.required && !s.others);
        let s = super::physical::evaluate(M::CMD, 0x10);
        assert!(s.required, "compound Cmd accepts right side");
    }

    #[test]
    fn typing_keys_are_not_valid_alone() {
        use handy_keys::Key;
        assert!(super::is_typing_key(&Key::A));
        assert!(super::is_typing_key(&Key::Space));
        assert!(!super::is_typing_key(&Key::F5));
    }
}
