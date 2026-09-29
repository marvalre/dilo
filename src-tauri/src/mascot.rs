//! The mascot window: a tiny transparent, click-through window that trails the
//! mouse cursor while you dictate. The face itself is drawn by `src/mascot`.

use enigo::{Enigo, Mouse, Settings};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "mascot";
const SIZE: f64 = 128.0;
/// Mascot center sits this far right/below the cursor tip.
const OFFSET_X: f64 = 48.0;
const OFFSET_Y: f64 = 26.0;
/// Fraction of the remaining distance covered each frame (trailing effect).
const FOLLOW: f64 = 0.35;
/// Time the swallow animation needs before the window hides (see mascot.css).
const SWALLOW_MS: u64 = 400;

/// Every `show` starts a new turn. Updates from an older turn (a dictation that
/// is still finishing while the user already started the next one) are ignored.
#[derive(Default)]
pub struct MascotState {
    turn: Mutex<u64>,
    follower: Mutex<Option<u64>>,
}

pub type Turn = u64;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("mascot.html".into()))
        .title("Dilo")
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
    let (mut x, mut y) = (cx + OFFSET_X - SIZE / 2.0, cy + OFFSET_Y - SIZE / 2.0);
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
            x = cx - OFFSET_X - SIZE / 2.0;
        }
        if y + SIZE > my + mh {
            y = cy - OFFSET_Y - SIZE / 2.0;
        }
        x = x.max(mx);
        y = y.max(my);
    }
    (x, y)
}

/// Shows the mascot for a new dictation and returns its turn.
pub fn show(app: &AppHandle) -> Turn {
    let state = app.state::<MascotState>();
    let turn = {
        let mut t = state.turn.lock().unwrap();
        *t += 1;
        *t
    };
    let Some(win) = app.get_webview_window(LABEL) else { return turn };
    if let Some((cx, cy)) = cursor() {
        let (x, y) = target_for(app, cx, cy);
        let _ = win.set_position(LogicalPosition::new(x, y));
    }
    let _ = app.emit_to(LABEL, "mascot://state", "listening");
    let _ = win.show();
    #[cfg(target_os = "macos")]
    let _ = win.set_always_on_top(true);

    // One follow thread per turn; it stops as soon as the turn changes or hides.
    *state.follower.lock().unwrap() = Some(turn);
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(enigo) = Enigo::new(&Settings::default()) else { return };
        let mut pos: Option<(f64, f64)> = None;
        while *app.state::<MascotState>().follower.lock().unwrap() == Some(turn) {
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
    turn
}

fn is_current(app: &AppHandle, turn: Turn) -> bool {
    *app.state::<MascotState>().turn.lock().unwrap() == turn
}

pub fn set_state(app: &AppHandle, turn: Turn, state: &str) {
    if is_current(app, turn) {
        let _ = app.emit_to(LABEL, "mascot://state", state);
    }
}

pub fn set_level(app: &AppHandle, level: f32) {
    let _ = app.emit_to(LABEL, "mascot://level", level);
}

/// Plays the swallow animation, then hides the window — unless a newer turn started.
pub fn swallow(app: &AppHandle, turn: Turn) {
    if !is_current(app, turn) {
        return;
    }
    let _ = app.emit_to(LABEL, "mascot://state", "swallow");
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(SWALLOW_MS));
        let state = app.state::<MascotState>();
        // Check and hide under the turn lock so a new `show` can't slip in between.
        let t = state.turn.lock().unwrap();
        if *t != turn {
            return;
        }
        *state.follower.lock().unwrap() = None;
        let _ = app.emit_to(LABEL, "mascot://state", "hidden");
        if let Some(win) = app.get_webview_window(LABEL) {
            let _ = win.hide();
        }
    });
}

/// Shows a face for a moment (sad / confused), then swallows — for this turn only.
pub fn flash_then_swallow(app: &AppHandle, turn: Turn, face: &'static str, ms: u64) {
    set_state(app, turn, face);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        swallow(&app, turn);
    });
}
