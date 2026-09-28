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

## 2b. v2 (2026-09-28 evening) — owner feedback after first real test

Owner dictated with it successfully, then asked for:
1. Mascot much smaller + "pro/Apple" look. References: Clicky (tiny cursor buddy) and Meta Muse's mascot "Jolly"
   (beady oval eyes, minimal smile). Mouth must be **sound-wave BARS** (like Siri/Voice Memos), not a sine line.
   → 4 skins in `src/shared/MascotFace.tsx`: **glass (default)**, jolly, dot, aura; sizes s/m/l (m ≈ 26–30 px).
2. A real app window "like Wispr Flow": sidebar (Inicio, Historial, Diccionario, Monito, Ajustes), shows in Dock while open.
3. Editable: history texts, personal dictionary (corrections), shortcuts (say X → paste Y), mascot appearance.
4. Storage bar on Home (model + history) with a warning threshold (setting `storage_warn_mb`).
5. Choose the dictation key by pressing it ("Cambiar" → `capture_hotkey` command → handy-keys KeyboardListener).

Backend for all of this is in: `rules.rs` (dictionary, tested), `store.rs` (rules table, update_text, clear, size_bytes),
`hotkey.rs::capture`, `commands.rs` (update_dictation, clear_history, storage_info, capture_hotkey, list_rules, save_rule,
delete_rule), `settings.rs` (mascot_skin, mascot_size, launch_at_login, storage_warn_mb), `lib.rs` (Dock policy, autostart).
Frontend contract: `src/panel/api.ts`. Events added: `settings://changed`.

**Signing (important):** build with `./scripts/build-local.sh` — it signs with the first local codesigning identity
so macOS keeps Accessibility/Mic grants across rebuilds. Plain `bun tauri build` = ad-hoc signature = the user must
re-grant Accessibility after every build (symptom: log says "Accessibility permission not granted", Fn does nothing).

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

- Transcription: 5.5 s Spanish audio in ~0.17–0.19 s; 4.0 s English in ~0.13–0.23 s. Perfect text on fixtures (punctuation, "a las 5").
- **Engine runs in a WORKER PROCESS** (`dicta --engine-worker <model_dir>`, see engine.rs header for the stdin/stdout protocol).
  Why: freeing the model in-process left ~500 MB retained by the allocator. Killing the worker returns 100%.
- App process RSS: ~85 MB idle, ~93 MB during use. Worker RSS while alive: ~1.37 GB. Worker starts in ~0.9 s (started on key press, so it loads while the user talks) and is killed after `idle_unload_min` (default 5).
- Idea to cut worker RAM (~1.37 GB): Handy now ships a GGUF Q4_K_M Parakeet (485 MB) run via `transcribe-cpp` (ggml, mmap). Switching the worker to that could roughly halve RAM. ORT session options are not exposed by transcribe-rs 0.3.11 (session.rs uses Level3 + defaults).
- E2E verified with `DICTA_SIMULATE_WAV=<16k wav> Dicta.app/Contents/MacOS/dicta`: mascot shows, text transcribed, pasted into the frontmost app, row saved in dicta.db. NOTE: it pastes into whatever app is frontmost — beware when testing.

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
- [x] Panel UI (src/panel), builds; reads initial tab from location.hash.
- [x] Engine moved to worker process (RAM), measured, README updated. LICENSE (MIT), README with credits.
- [x] E2E via DICTA_SIMULATE_WAV (transcribe → paste → history) works in the release bundle.
- [ ] **Human test**: open Dicta.app, grant Accessibility + Microphone, set 🌐 key to "Do nothing", hold Fn in TextEdit, speak, release. (Real mic + real Fn not yet tested by a human.)
- [ ] Visual check of the panel inside the real app (it was checked in a browser with mocked data only).
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
