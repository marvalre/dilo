# Dilo — Design Spec (2026-09-28)

> Working name "Dilo" is provisional. The final name is TBD; rename with search/replace.

## 1. What it is

An open-source, cross-platform (macOS first, then Windows, Linux) push-to-talk dictation app,
inspired by the dictation feature of Meta AI for Mac (hold Fn, speak, text lands at your cursor).

- Hold a hotkey (default: **Fn** on macOS, **Right Ctrl** on Windows/Linux) → speak → release.
- While you speak, a tiny **mascot ("el monito")** floats next to your mouse cursor. Its mouth is the
  live sound wave of your voice.
- On release, speech is transcribed **locally** (no cloud, no cost) and pasted where your text cursor is.
- The mascot "swallows itself" and disappears.
- A tray/menu-bar app (no Dock icon) opens a small panel with **History**, **Stats**, **Settings**.

Non-goals for v1: live text preview, LLM cleanup, cloud sync, accounts, streaming transcription.

## 2. Decisions (and why)

| Decision | Choice | Why |
|---|---|---|
| Stack | **Tauri v2** (Rust core + React/TS webviews) | Cross-platform, low RAM (tens of MB vs Electron's hundreds) |
| Relation to Handy | **New app** that uses Handy's published crates (`transcribe-rs`, `handy-keys`); code snippets borrowed with MIT attribution | Handy (MIT, ~44k LOC) already solves the hard parts; forking would drag in settings sprawl and upstream-merge cost. Handy's name/logo cannot be used by forks. |
| Default engine | **Parakeet TDT 0.6B v3 (int8 ONNX)** via `transcribe-rs` | Fastest accurate local model; auto-detects 25 European languages incl. Spanish/English code-switching |
| Other languages | **Whisper** (optional download) with explicit language selection | 99 languages; used when user picks a non-Parakeet language |
| RAM | Model loaded lazily on first dictation, **unloaded after N idle minutes** (default 10, setting) | "Don't eat RAM" requirement |
| Paste | Save clipboard → set text → synthesize Cmd/Ctrl+V → restore clipboard after 300 ms | Works in every app; same approach as Handy/Meta |
| Storage | SQLite (`rusqlite`, bundled) in the app data dir | History + stats, tiny footprint |

Licenses verified: transcribe-rs MIT, handy-keys MIT, enigo MIT, cpal Apache-2.0.

## 3. Architecture

Rust crate `src-tauri`, one module per responsibility:

| Module | Responsibility | Depends on |
|---|---|---|
| `hotkey` | Emits `Pressed` / `Released` for the configured key (incl. Fn on macOS) | `handy-keys` |
| `recorder` | Captures mic while held, downmixes to mono, resamples to 16 kHz f32; emits RMS level (~30 Hz) | `cpal`, `rubato` |
| `engine` | `Transcriber` trait; Parakeet impl; lazy load + idle unload | `transcribe-rs` |
| `paste` | Clipboard save/set/restore + key synthesis | `enigo`, `tauri-plugin-clipboard-manager` / `arboard` |
| `store` | SQLite schema, insert dictation, list history, compute stats (pure fns, unit tested) | `rusqlite` |
| `stats` | Pure functions: word counting, daily buckets, streak, WPM | none |
| `models` | Download + extract model with progress events; checks presence | `reqwest`, `tar`, `flate2` |
| `mascot` | Mascot window: create, follow cursor (60 Hz), send state/level events | `tauri`, `enigo` (cursor pos) |
| `coordinator` | State machine Idle → Recording → Transcribing → Pasting → Idle | all |
| `settings` | JSON settings (hotkey, language, model, idle-unload minutes, mic, mascot on/off) | `tauri-plugin-store` or serde file |
| `tray` | Tray icon + menu: Open panel, Language submenu, Quit | `tauri` |

Webviews (Vite + React + TS, one bundle, two entry HTMLs):
- `mascot.html` — the mascot (SVG + CSS/JS animation). Transparent, borderless, always-on-top, non-focusable,
  click-through (`set_ignore_cursor_events(true)`), skip taskbar.
- `index.html` — the panel with tabs History / Stats / Settings. Window is **destroyed on close** to free RAM.

### Dictation flow

1. `hotkey` Pressed → `coordinator` starts `recorder`, shows mascot at cursor (state `listening`).
2. `recorder` level events → mascot mouth (≈30 Hz). Mascot window follows cursor.
3. `hotkey` Released → if audio < 0.3 s or near-silent → cancel (mascot `swallow`, nothing pasted).
4. Else mascot state `thinking`; `engine` transcribes on a worker thread.
5. `paste` inserts text; mascot `done` → `swallow` animation → hide.
6. `store` inserts row: text, started_at, duration_ms, word_count, language, app_name (frontmost app when available).

### Errors
- Missing mic/accessibility permission → mascot `confused` face, panel opens on a Permissions screen.
- No model downloaded → panel opens on Model screen with a download button + progress.
- Engine error → mascot `sad`, entry saved with `error` text so user sees it in history.

## 4. The mascot ("el monito")

- **Body:** 44 px circle, warm cream `#F5EEE6` with soft radial highlight, 1 px ink outline `rgba(20,20,25,.18)`
  and soft drop shadow (visible on dark and light backgrounds).
- **Eyes:** two ink dots (`#1C1C22`, 4.5 px), 12 px apart, slightly above center.
  - Random **blink** every 2.5–5 s (eyes squash to lines for 120 ms).
  - `thinking`: eyes become short horizontal lines looking up-right, gentle bob.
  - `done`: happy eyes `^ ^` (arcs).
  - `confused`: one dot, one line; `sad`: arcs downward.
- **Mouth = sound wave:** SVG polyline, 16 px wide, 9 points, under the eyes.
  - Idle/silent: flat calm line (slight smile curve).
  - Speaking: each point's y = sin(phase + i·k) · amplitude · envelope(i), amplitude from smoothed RMS
    (attack 40 ms, release 120 ms). Center points move most (envelope bell curve).
- **Placement:** window 72×72 px; mascot center at cursor + (+28, +28) px; flipped to the other side
  when near a screen edge. Follows the cursor at 60 Hz with light easing (lerp 0.35) so it trails cutely.
- **Enter:** pop-in: scale 0.3 → 1 with spring (≈220 ms, slight overshoot), opacity 0 → 1.
- **Exit ("se traga a sí mismo"):** eyes and mouth are pulled into the center (scale → 0, 120 ms), then the
  body squashes (scaleX 1.15 / scaleY 0.85) and implodes to a 2 px dot with a slight twist (rotate 25°),
  ending at scale 0 (≈280 ms total, ease-in). Then the window hides.
- Respects "reduce motion": fades instead of springs/implosion.

## 5. Panel

Opened from the tray. ~420×560 px, rounded, dark/light aware.

- **History:** reverse-chronological list; each item: text (2 lines, expandable), time, words, duration,
  language; actions: copy, delete. Search box. Paginated (50).
- **Stats:**
  - Tiles: Total words · Words today · Avg words/day (over active days since first use) · Avg words/dictation ·
    Avg WPM · Current streak (days) · Total dictations · Time saved (words ÷ 40 wpm typing − speaking time).
  - 30-day bar chart of words per day.
  - Language breakdown (share of words).
- **Settings:** Language (Auto/Parakeet list, or pick a Whisper language), Hotkey, Model (download/delete),
  Microphone, Unload model after N minutes, Mascot on/off, Launch at login.

## 6. Data model

```sql
CREATE TABLE dictations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_at INTEGER NOT NULL,   -- unix ms (start of recording)
  duration_ms INTEGER NOT NULL,  -- audio length
  text TEXT NOT NULL,
  word_count INTEGER NOT NULL,
  language TEXT,                 -- ISO 639-1 if known
  app_name TEXT,                 -- frontmost app, if known
  error TEXT                     -- non-null if transcription failed
);
CREATE INDEX idx_dictations_created ON dictations(created_at);
```

Word counting: split on Unicode whitespace; tokens with at least one alphanumeric char count as words.
Days are bucketed in the user's local time zone.

## 7. Testing & acceptance
- Unit tests: `stats` (word count across es/en/emoji/punctuation, day buckets across midnight/TZ, streak,
  averages), `store` (insert/list/delete/stats on in-memory DB).
- Integration: transcribe bundled sample WAVs (es, en) with Parakeet; assert non-empty and key words present.
- Acceptance on the dev Mac (M5, macOS 26): idle RAM of core process reported; hold-to-paste works in
  TextEdit/Notes; latency release→paste measured and reported.

## 8. Roadmap after v1
Windows + Linux packaging and paste quirks (borrow from Handy `paste_tx/windows.rs`), launch-at-login,
auto-update, final name + logo, landing page, signed/notarized builds.
