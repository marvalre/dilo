# HANDOFF — Dicta (open-source dictation app)

> Written for the next AI model (GPT-6, Luna, Sol, Astra…) or human picking this up.
> Read this whole file before touching anything. Everything you need is here.
> Owner: Marcelo (speaks Spanish; answer him in Spanish, casual tone).

## 1. What this project is

A free, open-source clone of the **Meta AI for Mac dictation feature**:
hold a key (Fn on Mac) → talk → release → your words are pasted wherever your text cursor is.

Our twist: a **cute mascot ("el monito")** — a small cream ball with two dot eyes whose
**mouth is the live sound wave of your voice** — floats next to the mouse while you talk and
**swallows itself** (implodes) when done. Plus a tray app with **History**, **Stats**, **Settings**.

- Name "Dicta" is **provisional** (owner will decide later — rename with search/replace).
- Runs 100% locally (no cloud, free). Engine: NVIDIA **Parakeet TDT 0.6B v3** (int8 ONNX).
- Cross-platform goal: macOS first (done/being tested), then Windows, Linux.

## 2. Key decisions already made (do NOT re-litigate)

| Topic | Decision |
|---|---|
| Stack | Tauri v2 (Rust core + React/TS webviews), bun as JS package manager |
| Relation to Handy (github.com/cjpais/Handy, MIT, 32k★) | We did NOT fork. We use its published crates `transcribe-rs` (engine) and `handy-keys` (global hotkey). Handy's name/logo may NOT be used. README must credit Handy. |
| Engine | Parakeet v3 via `transcribe-rs` feature `onnx`. Auto-detects 25 European languages incl. es/en mixing. |
| RAM | Model loaded lazily (preloaded on key press) and unloaded after N idle minutes (default 10). |
| Mascot | Only waves as mouth (no live text). Design in spec §4. |
| Paste | clipboard save → set → Cmd/Ctrl+V → restore after 300 ms |
| Storage | SQLite `dicta.db` in app data dir |

Spec: `docs/superpowers/specs/2026-09-28-dicta-design.md`
Plan: `docs/superpowers/plans/2026-09-28-dicta-v1.md`

## 3. Where things are

```
dicta/
  HANDOFF.md                  ← this file (keep it updated!)
  docs/superpowers/specs|plans
  index.html / mascot.html    ← two Vite entry pages
  src/panel/                  ← panel UI (History / Stats / Settings). api.ts = TS↔Rust contract
  src/mascot/                 ← the mascot (Mascot.tsx SVG + mascot.css animations)
  assets/icon-1024.png        ← app icon source (regenerate icons: cd src-tauri && ../node_modules/.bin/tauri icon ../assets/icon-1024.png)
  src-tauri/
    Cargo.toml, tauri.conf.json, Info.plist (mic permission text, LSUIElement)
    icons/tray.png            ← menu-bar template icon (black silhouette)
    src/lib.rs                ← app setup (`run()`), `open_panel()`
    src/coordinator.rs        ← THE FLOW: on_press / on_release / finish; Core struct (shared state)
    src/engine.rs             ← Parakeet load/transcribe/unload
    src/recorder.rs           ← cpal mic capture → 16 kHz mono; level callback for the mouth
    src/hotkey.rs             ← handy-keys thread; press/release edges; accessibility helpers
    src/paste.rs              ← clipboard + Cmd+V; frontmost app name
    src/mascot.rs             ← mascot window (transparent, click-through) + cursor-follow thread
    src/store.rs, stats.rs    ← SQLite + pure stats (unit tested)
    src/settings.rs, models.rs, permissions.rs, tray.rs, commands.rs
    tests/engine_real.rs      ← real-model test (ignored by default)
    tests/fixtures/{es,en}.wav
```

Model files live in `~/Library/Application Support/com.dicta.app/models/parakeet-tdt-0.6b-v3-int8/`
(4 files from https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx). Already downloaded on Marcelo's Mac.

## 4. How to build / test / run

Rust is installed via rustup at `~/.cargo/bin` (NOT on PATH in some shells → call `~/.cargo/bin/cargo`).

```bash
cd "/Users/marcelo/M Visuals/CC Claude Code/dicta"
bun install
# unit tests (fast)
cd src-tauri && ~/.cargo/bin/cargo test --lib
# real engine test (needs model downloaded; prints latency)
~/.cargo/bin/cargo test --release --test engine_real -- --ignored --nocapture
# frontend type-check + build
cd .. && bun run build
# run app in dev (hot reload for the webviews)
PATH="$HOME/.cargo/bin:$PATH" bun tauri dev
# release bundle → src-tauri/target/release/bundle/macos/Dicta.app
PATH="$HOME/.cargo/bin:$PATH" bun tauri build --bundles app
```

Mascot preview in a normal browser (no Tauri needed — it runs a demo cycling all states):
`bun run dev` then open http://localhost:1420/mascot.html

## 5. Measured so far (Apple M5, macOS 26.6)

- Model load: ~0.56 s. Transcription: 5.5 s Spanish audio in ~0.19 s; 4.0 s English in ~0.13 s.
- Output quality: perfect on fixtures, adds punctuation, writes numbers as digits ("a las 5").

## 6. macOS permissions (the #1 source of "it doesn't work")

The app needs:
1. **Accessibility** (System Settings → Privacy & Security → Accessibility) — for the global Fn key AND for pasting (synthetic Cmd+V).
2. **Microphone** — system prompts on first recording.
3. **Fn key:** macOS may use Fn/🌐 for emoji picker or its own dictation. Tell the user: System Settings → Keyboard → "Press 🌐 key to" → **Do Nothing**.

Every rebuild of an unsigned binary can invalidate the Accessibility grant — if the hotkey stops working after a rebuild, remove Dicta from the Accessibility list and add it again.
An AI agent must NOT change system security settings itself; ask the human.

## 7. Status / TODO (update this list as you go)

Done:
- [x] Spec + plan
- [x] stats, store, settings, models, engine (+ tests, 15 unit tests passing)
- [x] recorder, paste, hotkey, mascot window + animations, coordinator, tray, commands (compiles)
- [x] App icon + tray icon (provisional mascot face)

In progress / next:
- [ ] Panel UI (src/panel) — built by a sub-agent; verify `bun run build` passes and that App.tsx reads the initial tab from `location.hash` (#history / #stats / #settings) because Rust opens `index.html#settings`.
- [ ] End-to-end manual test on Mac: hold Fn in TextEdit, speak, release → text pasted. Needs Accessibility granted by the human.
- [ ] Measure RAM (Activity Monitor or `ps -o rss= -p <pid>`) idle vs with model loaded; put numbers in README.
- [ ] README.md (es + en), LICENSE (MIT), credits.
- [ ] Windows/Linux: paste quirks (see Handy `src-tauri/src/paste_tx/windows.rs`), enigo cursor coordinates are physical pixels on Windows (mascot.rs assumes logical points), tray icon behavior.
- [ ] Launch at login (tauri-plugin-autostart), auto-update, signing/notarization.
- [ ] Final name + logo (owner decides).

## 8. Gotchas

- `cpal::Stream` is not Send on macOS → recorder keeps it on its own thread (see recorder.rs). Don't "simplify" that.
- Mascot window must stay `focusable(false)` + `set_ignore_cursor_events(true)`, otherwise it steals focus and the paste goes into the mascot instead of the user's app.
- The panel window is destroyed on close (RAM). The app keeps running because `RunEvent::ExitRequested` with `code: None` is prevented in lib.rs. Quit only via tray "Salir".
- `busy` flag in Core prevents overlapping dictations; always reset it (see on_press error paths).
- Events: Rust→mascot `mascot://state` (listening|thinking|done|swallow|sad|confused|hidden), `mascot://level` (0..1). Rust→panel `history://changed`, `model://status`, `panel://tab`.
- Keep `src/panel/api.ts` and `src-tauri/src/commands.rs` in sync (command names + arg names; Tauri converts camelCase JS args to snake_case Rust args automatically).
