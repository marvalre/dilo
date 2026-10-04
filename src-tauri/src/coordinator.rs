//! Push-to-talk flow: key down → record → key up → transcribe → paste → save.

use crate::recorder::{Recorder, Recording};
use crate::{engine::Engine, hotkey, mascot, models, paste, settings::Settings, store::*};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Recordings shorter than this are treated as accidental taps.
const MIN_MS: i64 = 300;
/// Below this RMS the recording is considered silence.
const MIN_PEAK_RMS: f32 = 0.01;
/// Keep listening this long after the key is released so the last word isn't clipped.
const TAIL: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Serialize, Default)]
pub struct ModelStatus {
    pub ready: bool,
    pub downloading: bool,
    pub done: u64,
    pub total: u64,
    pub loaded: bool,
    pub error: Option<String>,
}

pub struct Core {
    pub data_dir: PathBuf,
    pub store: Store,
    pub engine: Arc<Engine>,
    pub recorder: Recorder,
    pub settings: RwLock<Settings>,
    pub hotkey: Mutex<Option<hotkey::HotkeyHandle>>,
    pub model: Mutex<ModelStatus>,
    /// Dictations recorded but not yet pasted.
    jobs: AtomicUsize,
    /// Transcribe + paste one dictation at a time, in order.
    pipeline: Mutex<()>,
    /// True while the app is recording a new hotkey: dictation is paused.
    pub capturing: AtomicBool,
    press: Mutex<Option<Press>>,
}

struct Press {
    started_at: i64,
    target: paste::Target,
    turn: Option<mascot::Turn>,
}

impl Core {
    pub fn new(data_dir: PathBuf) -> anyhow::Result<Arc<Self>> {
        let settings = Settings::load(&data_dir);
        let store = Store::open(&data_dir.join("dilo.db"))?;
        let exe = std::env::current_exe()?;
        let engine = Engine::new(models::model_dir(&data_dir), exe);
        Ok(Arc::new(Self {
            data_dir,
            store,
            engine,
            recorder: Recorder::default(),
            settings: RwLock::new(settings),
            hotkey: Mutex::new(None),
            model: Mutex::new(ModelStatus::default()),
            jobs: AtomicUsize::new(0),
            pipeline: Mutex::new(()),
            capturing: AtomicBool::new(false),
            press: Mutex::new(None),
        }))
    }

    pub fn model_dir(&self) -> PathBuf {
        models::model_dir(&self.data_dir)
    }

    pub fn model_status(&self) -> ModelStatus {
        let mut s = lock(&self.model).clone();
        s.ready = models::is_ready(&self.model_dir());
        s.loaded = self.engine.is_loaded();
        s
    }
}

/// Poisoned locks are still usable: a panic in one dictation must not disable the next.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Decrements the pending-dictation counter even if the pipeline panics, so the
/// idle unloader is never blocked forever.
struct JobGuard<'a>(&'a AtomicUsize);
impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// True when a recording is too short or too quiet to be speech.
fn is_ignorable(duration_ms: i64, peak_rms: f32) -> bool {
    // `!(x >= y)` so a NaN level counts as silence.
    duration_ms < MIN_MS || !(peak_rms >= MIN_PEAK_RMS)
}

/// Longest a finished dictation waits for the user to let go of the hotkey
/// before pasting anyway (recordings are capped at MAX_SECONDS).
const MAX_PASTE_WAIT: Duration = Duration::from_secs(crate::recorder::MAX_SECONDS as u64 + 10);

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn on_hotkey(app: &AppHandle, pressed: bool) {
    let core = app.state::<Arc<Core>>().inner().clone();
    if pressed {
        on_press(app, &core);
    } else {
        on_release(app, core);
    }
}

fn on_press(app: &AppHandle, core: &Arc<Core>) {
    if core.capturing.load(Ordering::SeqCst) || core.recorder.is_recording() {
        return;
    }
    if !models::is_ready(&core.model_dir()) {
        crate::open_panel(app, Some("settings"));
        return;
    }
    let started_at = now_ms();
    let (mascot_on, mic) = {
        let s = core.settings.read().unwrap_or_else(|p| p.into_inner());
        (s.mascot_enabled, s.mic.clone())
    };

    // Open the mic first: every millisecond before this is speech we'd lose.
    let t = Instant::now();
    let level_app = app.clone();
    let on_level: crate::recorder::LevelFn = Arc::new(move |l| mascot::set_level(&level_app, l));
    let started = core.recorder.start(mic, on_level);
    log::info!("mic open in {:?}", t.elapsed());

    let turn = mascot_on.then(|| mascot::show(app));
    match started {
        Ok(()) => {
            *lock(&core.press) = Some(Press { started_at, target: paste::frontmost(), turn });
            // Load the model while the user talks so release → paste is fast.
            let engine = core.engine.clone();
            std::thread::spawn(move || {
                if let Err(e) = engine.preload() {
                    log::error!("preload: {e:#}");
                }
            });
        }
        Err(e) => {
            log::error!("recorder: {e:#}");
            if let Some(t) = turn {
                mascot::flash_then_swallow(app, t, "confused", 1200);
            }
            crate::open_panel(app, Some("settings"));
        }
    }
}

fn on_release(app: &AppHandle, core: Arc<Core>) {
    if !core.recorder.is_recording() {
        return;
    }
    // Stopping here (on the hotkey thread) means a quick next press finds the mic free.
    std::thread::sleep(TAIL);
    let press = lock(&core.press).take();
    let recording = core.recorder.stop();
    let press = press.unwrap_or(Press { started_at: now_ms(), target: paste::Target::default(), turn: None });
    let recording = match recording {
        Ok(r) => r,
        Err(e) => {
            log::error!("stop: {e:#}");
            if let Some(t) = press.turn {
                mascot::swallow(app, t);
            }
            return;
        }
    };
    core.jobs.fetch_add(1, Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        {
            let _job = JobGuard(&core.jobs);
            let _order = core.pipeline.lock().unwrap_or_else(|p| p.into_inner());
            complete(&app, &core, recording, press);
        }
    });
}

fn complete(app: &AppHandle, core: &Core, recording: Recording, press: Press) {
    let turn = press.turn;
    let face = |state: &str| {
        if let Some(t) = turn {
            mascot::set_state(app, t, state);
        }
    };
    let swallow = || {
        if let Some(t) = turn {
            mascot::swallow(app, t);
        }
    };
    if is_ignorable(recording.duration_ms, recording.peak_rms) {
        log::info!("ignored: {} ms, peak {:.4}", recording.duration_ms, recording.peak_rms);
        swallow();
        return;
    }
    if recording.truncated {
        log::warn!("recording cut at {} s", crate::recorder::MAX_SECONDS);
    }

    face("thinking");
    let t = Instant::now();
    let result = core.engine.transcribe(&recording.samples);
    log::info!("transcribed {} ms of audio in {:?}", recording.duration_ms, t.elapsed());

    let new = match result {
        Ok(text) if text.trim().is_empty() => {
            swallow();
            return;
        }
        Ok(raw) => {
            let rules = core.store.rules().unwrap_or_default();
            let text = crate::rules::apply(&raw, &rules);
            if text.trim().is_empty() {
                // A rule can erase everything (e.g. "um" -> ""): nothing to paste.
                swallow();
                return;
            }
            // Cmd+V while the hotkey is held (the user already started the next
            // dictation) would read as a different key combo and cut that
            // recording. Paste once they let go; the queue keeps the order.
            let waiting_since = Instant::now();
            while core.recorder.is_recording() && waiting_since.elapsed() < MAX_PASTE_WAIT {
                std::thread::sleep(Duration::from_millis(30));
            }
            let (pasted, restore) = paste::paste_text(&text, &press.target);
            if let Err(e) = pasted {
                log::error!("paste: {e:#}");
            }
            face("done");
            if let Some(r) = restore {
                r.finish();
            }
            swallow();
            let language = detect_language(&text, &core.settings.read().unwrap_or_else(|p| p.into_inner()).language);
            NewDictation {
                created_at: press.started_at,
                duration_ms: recording.duration_ms,
                text,
                language,
                app_name: press.target.name,
                error: None,
            }
        }
        Err(e) => {
            log::error!("{e:#}");
            if let Some(t) = turn {
                mascot::flash_then_swallow(app, t, "sad", 1200);
            }
            NewDictation {
                created_at: press.started_at,
                duration_ms: recording.duration_ms,
                text: String::new(),
                language: None,
                app_name: press.target.name,
                error: Some(format!("{e:#}")),
            }
        }
    };
    if let Err(e) = core.store.insert(new) {
        log::error!("store: {e:#}");
    }
    let _ = app.emit("history://changed", ());
}

/// Debug/QA: plays a 16 kHz mono WAV through the whole pipeline as if the user
/// had held the key and spoken it (mascot, transcription, paste, history).
pub fn simulate(app: &AppHandle, wav: &std::path::Path) -> anyhow::Result<()> {
    let core = app.state::<Arc<Core>>().inner().clone();
    let mut reader = hound::WavReader::open(wav)?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16_000 {
        anyhow::bail!("simulate needs a 16 kHz mono WAV (got {} ch, {} Hz)", spec.channels, spec.sample_rate);
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => reader.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?,
    };
    let started_at = now_ms();
    let target = paste::frontmost();
    let engine = core.engine.clone();
    std::thread::spawn(move || engine.preload());
    let turn = Some(mascot::show(app));
    let chunk = 16_000 / 30;
    let mut peak = 0f32;
    for c in samples.chunks(chunk) {
        let rms = (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt();
        peak = peak.max(rms);
        mascot::set_level(app, (rms * 12.0).min(1.0));
        std::thread::sleep(Duration::from_millis(33));
    }
    let duration_ms = samples.len() as i64 * 1000 / 16_000;
    let _order = core.pipeline.lock().unwrap_or_else(|p| p.into_inner());
    let rec = Recording { samples, duration_ms, peak_rms: peak, truncated: false };
    complete(app, &core, rec, Press { started_at, target, turn });
    Ok(())
}

/// The user's language setting wins; with "auto" we guess from the text, among
/// the languages the app offers, and only when the guess is reliable (short
/// phrases like "Ronda 1, primera frase." otherwise come out as Swedish).
fn detect_language(text: &str, setting: &str) -> Option<String> {
    use whatlang::Lang;
    if setting != "auto" {
        return Some(setting.to_string());
    }
    const LANGS: [(Lang, &str); 6] =
        [(Lang::Spa, "es"), (Lang::Eng, "en"), (Lang::Fra, "fr"), (Lang::Deu, "de"), (Lang::Por, "pt"), (Lang::Ita, "it")];
    let detector = whatlang::Detector::with_allowlist(LANGS.iter().map(|(l, _)| *l).collect());
    let info = detector.detect(text)?;
    if !info.is_reliable() {
        return None;
    }
    LANGS.iter().find(|(l, _)| *l == info.lang()).map(|(_, c)| c.to_string())
}

/// Periodically frees the model from RAM when idle.
pub fn spawn_idle_unloader(core: Arc<Core>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let min = core.settings.read().unwrap_or_else(|p| p.into_inner()).idle_unload_min;
        if min > 0 && core.jobs.load(Ordering::SeqCst) == 0 && !core.recorder.is_recording() {
            core.engine.unload_if_idle(Duration::from_secs(u64::from(min) * 60));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_setting_overrides_detection() {
        assert_eq!(detect_language("hello there my friend", "es").as_deref(), Some("es"));
        assert_eq!(
            detect_language("Mañana tengo una reunión muy importante con el equipo", "auto").as_deref(),
            Some("es")
        );
        assert_ne!(detect_language("Ronda 1, segunda frase.", "auto").as_deref(), Some("sv"));
        assert_eq!(
            detect_language("I need to finish the quarterly report before the meeting tomorrow", "auto").as_deref(),
            Some("en")
        );
    }

    #[test]
    fn short_quiet_or_nan_recordings_are_ignored() {
        assert!(is_ignorable(100, 0.5));
        assert!(is_ignorable(299, 0.5));
        assert!(!is_ignorable(300, 0.5));
        assert!(is_ignorable(1000, 0.0));
        assert!(is_ignorable(1000, 0.0099));
        assert!(!is_ignorable(1000, 0.01));
        assert!(is_ignorable(1000, f32::NAN));
        assert!(is_ignorable(0, 0.0));
        assert!(is_ignorable(-5, 1.0));
    }

    #[test]
    fn job_guard_decrements_even_on_panic() {
        let n = Arc::new(AtomicUsize::new(1));
        let n2 = n.clone();
        let r = std::thread::spawn(move || {
            let _g = JobGuard(&n2);
            panic!("pipeline blew up");
        })
        .join();
        assert!(r.is_err());
        assert_eq!(n.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn detection_handles_empty_and_symbols() {
        assert_eq!(detect_language("", "auto"), None);
        assert_eq!(detect_language("12345 !!! ???", "auto"), None);
        assert_eq!(detect_language("", "es").as_deref(), Some("es"));
    }

    #[test]
    fn lock_survives_poisoning() {
        let m = Arc::new(Mutex::new(5));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("x");
        })
        .join();
        assert_eq!(*lock(&m), 5);
    }
}
