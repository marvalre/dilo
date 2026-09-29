//! Put text at the user's cursor: snapshot the clipboard, set our text, press
//! Cmd/Ctrl+V, then restore everything that was on the clipboard before.

use anyhow::{anyhow, Result};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// The app that had focus when the user pressed the hotkey.
#[derive(Debug, Clone, Default)]
pub struct Target {
    pub name: Option<String>,
    pub pid: Option<i32>,
}

/// Time the target app gets to read the clipboard before we restore it.
const RESTORE_AFTER: Duration = Duration::from_millis(600);

/// The user's clipboard, to put back once the target app has read our text.
pub struct Restore {
    saved: clip::Snapshot,
    ours: isize,
    at: Instant,
}

impl Restore {
    pub fn finish(self) {
        let wait = RESTORE_AFTER.saturating_sub(self.at.elapsed());
        sleep(wait);
        // Only restore if nobody (the user or another app) changed the clipboard meanwhile.
        if clip::change_count() == self.ours {
            clip::restore(self.saved);
        }
    }
}

/// Pastes `text` into `target` and returns the pending clipboard restore.
/// The restore still happens (via the returned value) when the key press fails.
pub fn paste_text(text: &str, target: &Target) -> (Result<()>, Option<Restore>) {
    refocus(target);
    let saved = clip::snapshot();
    let ours = match clip::set_text(&format!("{text} ")) {
        Ok(n) => n,
        Err(e) => return (Err(e), None),
    };
    sleep(Duration::from_millis(40));
    let pasted = press_paste();
    (pasted, Some(Restore { saved, ours, at: Instant::now() }))
}

/// If the user switched apps while we transcribed, go back to where they dictated.
fn refocus(target: &Target) {
    let Some(pid) = target.pid else { return };
    if frontmost().pid == Some(pid) {
        return;
    }
    if !activate(pid) {
        return;
    }
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline && frontmost().pid != Some(pid) {
        sleep(Duration::from_millis(20));
    }
    sleep(Duration::from_millis(60));
}

/// Cmd+V as one V key event carrying the Command flag, from a private event
/// source: no separate Cmd press, so the keyboard's global modifier state (and
/// the push-to-talk key the user may be holding) is left untouched.
#[cfg(target_os = "macos")]
fn press_paste() -> Result<()> {
    use std::ffi::c_void;
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceCreate(state_id: i32) -> *mut c_void;
        fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, down: bool) -> *mut c_void;
        fn CGEventSetFlags(event: *mut c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut c_void);
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *mut c_void);
    }
    const PRIVATE_STATE: i32 = -1;
    const HID_TAP: u32 = 0;
    const V: u16 = 9; // kVK_ANSI_V, layout independent
    const COMMAND: u64 = 0x0010_0000;
    unsafe {
        let src = CGEventSourceCreate(PRIVATE_STATE);
        if src.is_null() {
            return Err(anyhow!("no event source"));
        }
        for down in [true, false] {
            let ev = CGEventCreateKeyboardEvent(src, V, down);
            if ev.is_null() {
                CFRelease(src);
                return Err(anyhow!("no key event"));
            }
            CGEventSetFlags(ev, COMMAND);
            CGEventPost(HID_TAP, ev);
            CFRelease(ev);
        }
        CFRelease(src);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn press_paste() -> Result<()> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| anyhow!("enigo: {e:?}"))?;
    #[cfg(target_os = "windows")]
    let (modifier, v) = (Key::Control, Key::V);
    #[cfg(not(target_os = "windows"))]
    let (modifier, v) = (Key::Control, Key::Unicode('v'));
    enigo.key(modifier, Direction::Press).map_err(|e| anyhow!("{e:?}"))?;
    let r = enigo.key(v, Direction::Click).map_err(|e| anyhow!("{e:?}"));
    enigo.key(modifier, Direction::Release).map_err(|e| anyhow!("{e:?}"))?;
    r
}

pub fn frontmost() -> Target {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;
        let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else { return Target::default() };
        Target { name: app.localizedName().map(|n| n.to_string()), pid: Some(app.processIdentifier()) }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Target::default()
    }
}

fn activate(pid: i32) -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .map(|app| app.activateWithOptions(NSApplicationActivationOptions::empty()))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        false
    }
}

#[cfg(target_os = "macos")]
mod clip {
    use anyhow::{bail, Result};
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
    use objc2_foundation::{NSArray, NSData, NSString};

    /// Every item on the clipboard with all of its representations.
    pub struct Snapshot(Vec<Vec<(Retained<NSString>, Retained<NSData>)>>);

    pub fn snapshot() -> Snapshot {
        let pb = NSPasteboard::generalPasteboard();
        let items = pb.pasteboardItems().map(|a| a.to_vec()).unwrap_or_default();
        Snapshot(
            items
                .iter()
                .map(|item| {
                    item.types()
                        .to_vec()
                        .into_iter()
                        .filter_map(|t| item.dataForType(&t).map(|d| (t, d)))
                        .collect()
                })
                .collect(),
        )
    }

    fn write(items: Vec<Retained<NSPasteboardItem>>) -> bool {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        if items.is_empty() {
            return true;
        }
        let objs: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> =
            items.into_iter().map(ProtocolObject::from_retained).collect();
        pb.writeObjects(&NSArray::from_retained_slice(&objs))
    }

    /// Sets plain text, marked transient so clipboard managers skip it. Returns the change count.
    pub fn set_text(text: &str) -> Result<isize> {
        let item = NSPasteboardItem::new();
        let data = NSData::with_bytes(text.as_bytes());
        item.setData_forType(&data, unsafe { NSPasteboardTypeString });
        item.setData_forType(&NSData::new(), &NSString::from_str("org.nspasteboard.TransientType"));
        if !write(vec![item]) {
            bail!("no se pudo escribir en el portapapeles");
        }
        Ok(change_count())
    }

    pub fn restore(s: Snapshot) {
        let items = s
            .0
            .into_iter()
            .map(|reps| {
                let item = NSPasteboardItem::new();
                for (t, d) in reps {
                    item.setData_forType(&d, &t);
                }
                item
            })
            .collect();
        write(items);
    }

    pub fn change_count() -> isize {
        NSPasteboard::generalPasteboard().changeCount()
    }
}

#[cfg(not(target_os = "macos"))]
mod clip {
    use anyhow::Result;
    pub struct Snapshot(Option<String>);
    pub fn snapshot() -> Snapshot {
        Snapshot(arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()))
    }
    pub fn set_text(text: &str) -> Result<isize> {
        arboard::Clipboard::new()?.set_text(text.to_string())?;
        Ok(0)
    }
    pub fn restore(s: Snapshot) {
        if let (Some(t), Ok(mut c)) = (s.0, arboard::Clipboard::new()) {
            let _ = c.set_text(t);
        }
    }
    pub fn change_count() -> isize {
        0
    }
}
