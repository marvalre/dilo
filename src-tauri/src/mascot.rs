//! The mascot window: a tiny transparent, click-through window that trails the
//! mouse cursor while you dictate. The face itself is drawn by `src/mascot`.

use enigo::{Enigo, Mouse, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "mascot";
const SIZE: f64 = 72.0;
/// Mascot center sits this far right/below the cursor tip.
const OFFSET: f64 = 30.0;
/// Fraction of the remaining distance covered each frame (trailing effect).
const FOLLOW: f64 = 0.35;
/// Time the swallow animation needs before the window hides (see mascot.css).
const SWALLOW_MS: u64 = 400;

#[derive(Default)]
pub struct MascotState {
    visible: Arc<AtomicBool>,
    generation: Arc<std::sync::atomic::AtomicU64>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("mascot.html".into()))
        .title("Dicta")
        .inner_size(SIZE, SIZE)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .visible_on_all_workspaces(true)
        .build()?;
    win.set_ignore_cursor_events(true)?;
    app.manage(MascotState::default());
    Ok(())
}

fn cursor_with(enigo: &Enigo) -> Option<(f64, f64)> {
    enigo.location().ok().map(|(x, y)| (x as f64, y as f64))
}

fn cursor() -> Option<(f64, f64)> {
    cursor_with(&Enigo::new(&Settings::default()).ok()?)
}

/// Top-left window position for a cursor, flipped away from screen edges.
fn target_for(app: &AppHandle, cx: f64, cy: f64) -> (f64, f64) {
    let (mut x, mut y) = (cx + OFFSET - SIZE / 2.0, cy + OFFSET - SIZE / 2.0);
    let monitor = app
        .available_monitors()
        .ok()
        .and_then(|ms| {
            ms.into_iter().find(|m| {
                let s = m.scale_factor();
                let (mx, my) = (m.position().x as f64 / s, m.position().y as f64 / s);
                let (mw, mh) = (m.size().width as f64 / s, m.size().height as f64 / s);
                cx >= mx && cx < mx + mw && cy >= my && cy < my + mh
            })
        });
    if let Some(m) = monitor {
        let s = m.scale_factor();
        let (mx, my) = (m.position().x as f64 / s, m.position().y as f64 / s);
        let (mw, mh) = (m.size().width as f64 / s, m.size().height as f64 / s);
        if x + SIZE > mx + mw {
            x = cx - OFFSET - SIZE / 2.0;
        }
        if y + SIZE > my + mh {
            y = cy - OFFSET - SIZE / 2.0;
        }
        x = x.max(mx);
        y = y.max(my);
    }
    (x, y)
}

pub fn show(app: &AppHandle) {
    let Some(win) = app.get_webview_window(LABEL) else { return };
    let state = app.state::<MascotState>();
    let gen = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some((cx, cy)) = cursor() {
        let (x, y) = target_for(app, cx, cy);
        let _ = win.set_position(LogicalPosition::new(x, y));
    }
    set_state(app, "listening");
    let _ = win.show();
    #[cfg(target_os = "macos")]
    let _ = win.set_always_on_top(true);

    if state.visible.swap(true, Ordering::SeqCst) {
        return; // follow thread already running
    }
    let (app, visible, generation) = (app.clone(), state.visible.clone(), state.generation.clone());
    std::thread::spawn(move || {
        let Ok(enigo) = Enigo::new(&Settings::default()) else { return };
        let mut pos: Option<(f64, f64)> = None;
        while visible.load(Ordering::SeqCst) && generation.load(Ordering::SeqCst) >= gen {
            if let Some((cx, cy)) = cursor_with(&enigo) {
                let (tx, ty) = target_for(&app, cx, cy);
                let (x, y) = match pos {
                    Some((px, py)) => (px + (tx - px) * FOLLOW, py + (ty - py) * FOLLOW),
                    None => (tx, ty),
                };
                if pos.map_or(true, |(px, py)| (px - x).abs() > 0.3 || (py - y).abs() > 0.3) {
                    let _ = win.set_position(LogicalPosition::new(x, y));
                }
                pos = Some((x, y));
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    });
}

pub fn set_state(app: &AppHandle, state: &str) {
    let _ = app.emit_to(LABEL, "mascot://state", state);
}

pub fn set_level(app: &AppHandle, level: f32) {
    let _ = app.emit_to(LABEL, "mascot://level", level);
}

/// Plays the swallow animation, then hides the window.
pub fn swallow(app: &AppHandle) {
    set_state(app, "swallow");
    let state = app.state::<MascotState>();
    let gen = state.generation.load(Ordering::SeqCst);
    let (app, visible, generation) = (app.clone(), state.visible.clone(), state.generation.clone());
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(SWALLOW_MS));
        // A new dictation started meanwhile: keep the mascot.
        if generation.load(Ordering::SeqCst) != gen {
            return;
        }
        visible.store(false, Ordering::SeqCst);
        set_state(&app, "hidden");
        if let Some(win) = app.get_webview_window(LABEL) {
            let _ = win.hide();
        }
    });
}

/// Shows a face for a moment (sad / confused), then swallows.
pub fn flash_then_swallow(app: &AppHandle, face: &'static str, ms: u64) {
    set_state(app, face);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        swallow(&app);
    });
}
