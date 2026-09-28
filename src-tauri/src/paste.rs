//! Put text at the user's cursor: save clipboard, set text, press Cmd/Ctrl+V, restore.

use anyhow::{anyhow, Result};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread::sleep;
use std::time::Duration;

pub fn paste_text(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    let previous = clipboard.get_text().ok();

    clipboard.set_text(format!("{text} "))?;
    sleep(Duration::from_millis(40));
    press_paste()?;
    // Give the target app time to read the clipboard before restoring it.
    sleep(Duration::from_millis(300));
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    Ok(())
}

fn press_paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| anyhow!("enigo: {e:?}"))?;
    #[cfg(target_os = "macos")]
    let (modifier, v) = (Key::Meta, Key::Other(9)); // kVK_ANSI_V, layout independent
    #[cfg(target_os = "windows")]
    let (modifier, v) = (Key::Control, Key::V);
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let (modifier, v) = (Key::Control, Key::Unicode('v'));

    enigo.key(modifier, Direction::Press).map_err(|e| anyhow!("{e:?}"))?;
    let r = enigo.key(v, Direction::Click).map_err(|e| anyhow!("{e:?}"));
    enigo.key(modifier, Direction::Release).map_err(|e| anyhow!("{e:?}"))?;
    r
}

/// Name of the app that currently has focus (where the text will land).
pub fn frontmost_app() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;
        #[allow(unused_unsafe)]
        unsafe {
            let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
            app.localizedName().map(|n| n.to_string())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}
