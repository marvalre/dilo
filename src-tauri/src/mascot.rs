//! The mascot window: a tiny transparent, click-through window that trails the
//! mouse cursor while you dictate. The face itself is drawn by `src/mascot`.

use enigo::{Enigo, Mouse, Settings};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

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
    app.manage(MascotState::default());
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
    Ok(())
}

fn cursor_with(enigo: &Enigo) -> Option<(f64, f64)> {
    enigo.location().ok().map(|(x, y)| (x as f64, y as f64))
}

fn cursor() -> Option<(f64, f64)> {
    cursor_with(&Enigo::new(&Settings::default()).ok()?)
}

/// A monitor's rectangle in the same units as the cursor.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Monitor rectangles plus scale. enigo reports the cursor in logical points on macOS
/// and in physical pixels on Windows/Linux, so everything here is converted to that space.
type Monitors = Vec<(Rect, f64)>;

fn monitors(app: &AppHandle) -> Monitors {
    app.available_monitors()
        .map(|ms| {
            ms.into_iter()
                .map(|m| {
                    let s = if cfg!(target_os = "macos") { m.scale_factor().max(0.1) } else { 1.0 };
                    let r = Rect {
                        x: m.position().x as f64 / s,
                        y: m.position().y as f64 / s,
                        w: m.size().width as f64 / s,
                        h: m.size().height as f64 / s,
                    };
                    (r, m.scale_factor())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The monitor containing the cursor, or the nearest one when it sits on a gap or a
/// dead edge between screens (negative coordinates and mixed layouts included).
fn monitor_at(ms: &[(Rect, f64)], cx: f64, cy: f64) -> Option<(Rect, f64)> {
    let dist = |r: &Rect| {
        let dx = (r.x - cx).max(0.0).max(cx - (r.x + r.w));
        let dy = (r.y - cy).max(0.0).max(cy - (r.y + r.h));
        dx * dx + dy * dy
    };
    ms.iter().copied().min_by(|a, b| dist(&a.0).total_cmp(&dist(&b.0)))
}

/// Top-left window position for a cursor: below-right of it, flipped to the other
/// side near the monitor's right/bottom edge and always kept inside the monitor.
fn place(cx: f64, cy: f64, size: f64, off_x: f64, off_y: f64, monitor: Option<Rect>) -> (f64, f64) {
    let (mut x, mut y) = (cx + off_x - size / 2.0, cy + off_y - size / 2.0);
    if let Some(m) = monitor {
        if x + size > m.x + m.w {
            x = cx - off_x - size / 2.0;
        }
        if y + size > m.y + m.h {
            y = cy - off_y - size / 2.0;
        }
        // Clamp (right/bottom first so the left/top edge wins on tiny monitors).
        x = x.min(m.x + m.w - size).max(m.x);
        y = y.min(m.y + m.h - size).max(m.y);
    }
    (x, y)
}

/// Window position for a cursor, in the cursor's units (see `Monitors`).
fn target_for(ms: &[(Rect, f64)], cx: f64, cy: f64) -> (f64, f64) {
    let found = monitor_at(ms, cx, cy);
    // On macOS the window is positioned in logical points; elsewhere in physical
    // pixels, so size and offsets grow with the monitor's scale.
    let k = if cfg!(target_os = "macos") { 1.0 } else { found.map_or(1.0, |m| m.1) };
    place(cx, cy, SIZE * k, OFFSET_X * k, OFFSET_Y * k, found.map(|m| m.0))
}

fn set_pos(win: &tauri::WebviewWindow, x: f64, y: f64) {
    if cfg!(target_os = "macos") {
        let _ = win.set_position(LogicalPosition::new(x, y));
    } else {
        let _ = win.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Shows the mascot for a new dictation and returns its turn.
pub fn show(app: &AppHandle) -> Turn {
    let state = app.state::<MascotState>();
    let turn = {
        let mut t = lock(&state.turn);
        *t += 1;
        *t
    };
    let Some(win) = app.get_webview_window(LABEL) else { return turn };
    if let Some((cx, cy)) = cursor() {
        let (x, y) = target_for(&monitors(app), cx, cy);
        set_pos(&win, x, y);
    }
    let _ = app.emit_to(LABEL, "mascot://state", "listening");
    let _ = win.show();
    #[cfg(target_os = "macos")]
    let _ = win.set_always_on_top(true);

    // One follow thread per turn; it stops as soon as the turn changes or hides.
    *lock(&state.follower) = Some(turn);
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(enigo) = Enigo::new(&Settings::default()) else { return };
        let mut pos: Option<(f64, f64)> = None;
        // Monitors are re-read now and then (displays can be plugged in mid-dictation),
        // not every frame.
        let mut ms = monitors(&app);
        let mut ms_at = Instant::now();
        while *lock(&app.state::<MascotState>().follower) == Some(turn) {
            if ms_at.elapsed() > Duration::from_secs(1) {
                ms = monitors(&app);
                ms_at = Instant::now();
            }
            if let Some((cx, cy)) = cursor_with(&enigo) {
                let (tx, ty) = target_for(&ms, cx, cy);
                let (x, y) = match pos {
                    Some((px, py)) => (px + (tx - px) * FOLLOW, py + (ty - py) * FOLLOW),
                    None => (tx, ty),
                };
                if pos.map_or(true, |(px, py)| (px - x).abs() > 0.3 || (py - y).abs() > 0.3) {
                    set_pos(&win, x, y);
                }
                pos = Some((x, y));
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    });
    turn
}

fn is_current(app: &AppHandle, turn: Turn) -> bool {
    *lock(&app.state::<MascotState>().turn) == turn
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
        let t = lock(&state.turn);
        if *t != turn {
            return;
        }
        *lock(&state.follower) = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect { x: 0.0, y: 0.0, w: 1000.0, h: 800.0 };

    #[test]
    fn places_below_right_of_cursor() {
        let (x, y) = place(300.0, 300.0, 128.0, 48.0, 26.0, Some(R));
        assert_eq!((x, y), (300.0 + 48.0 - 64.0, 300.0 + 26.0 - 64.0));
    }

    #[test]
    fn flips_at_right_and_bottom_edges() {
        let (x, y) = place(990.0, 790.0, 128.0, 48.0, 26.0, Some(R));
        assert_eq!((x, y), (990.0 - 48.0 - 64.0, 790.0 - 26.0 - 64.0));
        assert!(x + 128.0 <= 1000.0 && y + 128.0 <= 800.0);
    }

    #[test]
    fn stays_inside_monitor_with_negative_origin() {
        let left = Rect { x: -1920.0, y: -200.0, w: 1920.0, h: 1080.0 };
        // Cursor at the monitor's top-left corner: clamped, not pushed to x < -1920.
        let (x, y) = place(-1919.0, -199.0, 128.0, 48.0, 26.0, Some(left));
        assert!(x >= -1920.0 && y >= -200.0, "{x},{y}");
        // Cursor at its far right edge: flips instead of spilling onto the next monitor.
        let (x, _) = place(-2.0, 100.0, 128.0, 48.0, 26.0, Some(left));
        assert!(x + 128.0 <= 0.0, "{x}");
    }

    #[test]
    fn tiny_monitor_never_leaves_origin() {
        let tiny = Rect { x: 10.0, y: 10.0, w: 50.0, h: 50.0 };
        assert_eq!(place(30.0, 30.0, 128.0, 48.0, 26.0, Some(tiny)), (10.0, 10.0));
    }

    #[test]
    fn picks_monitor_containing_cursor_or_nearest() {
        let a = Rect { x: -1920.0, y: 0.0, w: 1920.0, h: 1080.0 };
        let b = Rect { x: 0.0, y: 0.0, w: 2560.0, h: 1440.0 };
        let ms = vec![(a, 1.0), (b, 2.0)];
        assert_eq!(monitor_at(&ms, -5.0, 10.0).unwrap().0, a);
        assert_eq!(monitor_at(&ms, 5.0, 10.0).unwrap().0, b);
        // Below monitor a (outside every monitor): nearest wins.
        assert_eq!(monitor_at(&ms, -100.0, 1200.0).unwrap().0, a);
        assert!(monitor_at(&[], 0.0, 0.0).is_none());
    }

    #[test]
    fn no_monitor_info_still_places() {
        assert_eq!(target_for(&[], 100.0, 100.0), place(100.0, 100.0, SIZE, OFFSET_X, OFFSET_Y, None));
    }
}
