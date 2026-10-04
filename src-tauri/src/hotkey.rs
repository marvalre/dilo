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
    let initial = hotkey.to_string();

    std::thread::Builder::new().name("hotkey".into()).spawn(move || {
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
        // What is currently active: the key string, its registration (None in polling
        // mode) and the modifiers to poll (None when registered with the manager).
        let mut active: Option<String> = None;
        let mut current: Option<HotkeyId> = None;
        let mut poll: Option<Modifiers> = None;
        // Switches to `hotkey`. If the new key can't be registered the previous one
        // stays active, so a bad setting never leaves the user without a hotkey.
        let activate = |hotkey: &str,
                        active: &mut Option<String>,
                        current: &mut Option<HotkeyId>,
                        poll: &mut Option<Modifiers>| {
            if active.as_deref() == Some(hotkey) {
                return;
            }
            if let Some(mods) = modifier_only(hotkey) {
                if let Some(id) = current.take() {
                    let _ = manager.unregister(id);
                }
                *poll = Some(mods);
                *active = Some(hotkey.to_string());
                log::info!("hotkey registered: {hotkey} (physical state)");
            } else if let Some(id) = register(&manager, hotkey) {
                if let Some(old) = current.replace(id) {
                    let _ = manager.unregister(old);
                }
                *poll = None;
                *active = Some(hotkey.to_string());
            } else {
                log::error!("hotkey {hotkey} not usable; keeping {:?}", active);
            }
        };
        activate(pending.as_deref().unwrap_or(&initial), &mut active, &mut current, &mut poll);
        let mut held = false;
        loop {
            match rx.try_recv() {
                Ok(mut next) => {
                    while let Ok(newer) = rx.try_recv() {
                        next = newer;
                    }
                    let before = active.clone();
                    activate(&next, &mut active, &mut current, &mut poll);
                    if held && active != before {
                        // The old key is no longer watched, so its release will never arrive.
                        held = false;
                        on_event(false);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // The handle was dropped: stop watching, and never leave a press dangling.
                    if let Some(id) = current.take() {
                        let _ = manager.unregister(id);
                    }
                    if held {
                        on_event(false);
                    }
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => {}
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
                    let next = next_held(held, state.required, state.others);
                    if next != held {
                        held = next;
                        on_event(next);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })?;

    Ok(HotkeyHandle { tx })
}

/// Edge detection for modifier-only keys: starting needs exactly the hotkey's
/// modifiers (so Cmd+Fn doesn't trigger "Fn"), but once held it only needs to
/// keep them down, so adding another modifier mid-dictation doesn't cut it off.
fn next_held(held: bool, required: bool, others: bool) -> bool {
    if held {
        required
    } else {
        required && !others
    }
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
    use handy_keys::KeyboardListener;
    // The listener is dropped on every exit path, which stops its event tap.
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
        match capture_step(&mut best, ev.key, ev.is_key_down, ev.modifiers) {
            CaptureStep::Continue => {}
            CaptureStep::Cancel => return Ok(None),
            CaptureStep::Done(hotkey) => return Ok(Some(format_hotkey(&hotkey))),
            CaptureStep::Invalid(e) => return Err(anyhow::anyhow!("{e}")),
        }
    }
}

#[derive(Debug, PartialEq)]
enum CaptureStep {
    Continue,
    Cancel,
    Done(Hotkey),
    Invalid(String),
}

/// One event of the capture state machine (pure, so it can be tested).
fn capture_step(best: &mut Modifiers, key: Option<handy_keys::Key>, is_down: bool, mods: Modifiers) -> CaptureStep {
    use handy_keys::Key;
    match key {
        Some(Key::Escape) if is_down => return CaptureStep::Cancel,
        // A bare letter, digit or space would start dictating on every keystroke;
        // so would Shift/Option + letter, which type capitals and accents.
        Some(key) if is_down && is_typing_key(&key) && !has_command_modifier(mods) => {}
        Some(key) if is_down => {
            return match Hotkey::new(generic_sides(mods), Some(key)) {
                Ok(h) => CaptureStep::Done(h),
                Err(e) => CaptureStep::Invalid(e.to_string()),
            };
        }
        _ => {}
    }
    if mods.bits().count_ones() > best.bits().count_ones() {
        *best = mods;
    }
    if mods.is_empty() && !best.is_empty() {
        return match Hotkey::new(*best, None) {
            Ok(h) => CaptureStep::Done(h),
            Err(e) => CaptureStep::Invalid(e.to_string()),
        };
    }
    CaptureStep::Continue
}

/// Ctrl, Cmd/Win or Fn: modifiers that don't change which character a key types.
fn has_command_modifier(m: Modifiers) -> bool {
    m.intersects(Modifiers::CTRL | Modifiers::CMD | Modifiers::FN)
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
    fn modifier_hold_edges() {
        use super::next_held;
        assert!(next_held(false, true, false), "exact modifiers start");
        assert!(!next_held(false, true, true), "extra modifier blocks the start");
        assert!(next_held(true, true, true), "extra modifier doesn't cut a held key");
        assert!(!next_held(true, false, false), "releasing stops");
        assert!(!next_held(false, false, false));
    }

    #[test]
    fn capture_modifier_only_returns_on_release() {
        use super::{capture_step, CaptureStep};
        use handy_keys::{Hotkey, Modifiers as M};
        let mut best = M::empty();
        assert_eq!(capture_step(&mut best, None, true, M::FN), CaptureStep::Continue);
        assert_eq!(capture_step(&mut best, None, true, M::FN | M::SHIFT_LEFT), CaptureStep::Continue);
        // Releasing one at a time keeps the largest set seen.
        assert_eq!(capture_step(&mut best, None, false, M::FN), CaptureStep::Continue);
        let done = capture_step(&mut best, None, false, M::empty());
        assert_eq!(done, CaptureStep::Done(Hotkey::new(M::FN | M::SHIFT_LEFT, None).unwrap()));
    }

    #[test]
    fn capture_regular_keys() {
        use super::{capture_step, CaptureStep};
        use handy_keys::{Hotkey, Key, Modifiers as M};
        let mut best = M::empty();
        assert_eq!(capture_step(&mut best, Some(Key::Escape), true, M::empty()), CaptureStep::Cancel);
        // Bare letters and Shift/Option + letter would hijack typing.
        assert_eq!(capture_step(&mut best, Some(Key::A), true, M::empty()), CaptureStep::Continue);
        assert_eq!(capture_step(&mut best, Some(Key::A), true, M::SHIFT_LEFT), CaptureStep::Continue);
        assert_eq!(capture_step(&mut best, Some(Key::A), true, M::OPT_RIGHT), CaptureStep::Continue);
        // Key-up of a regular key is not a capture.
        let mut fresh = M::empty();
        assert_eq!(capture_step(&mut fresh, Some(Key::F5), false, M::empty()), CaptureStep::Continue);
        assert_eq!(
            capture_step(&mut fresh, Some(Key::F5), true, M::empty()),
            CaptureStep::Done(Hotkey::new(M::empty(), Some(Key::F5)).unwrap())
        );
        assert_eq!(
            capture_step(&mut fresh, Some(Key::D), true, M::CMD_RIGHT | M::SHIFT_LEFT),
            CaptureStep::Done(Hotkey::new(M::CMD | M::SHIFT, Some(Key::D)).unwrap())
        );
    }

    #[test]
    fn typing_keys_are_not_valid_alone() {
        use handy_keys::Key;
        assert!(super::is_typing_key(&Key::A));
        assert!(super::is_typing_key(&Key::Space));
        assert!(!super::is_typing_key(&Key::F5));
    }
}
