//! Push-to-talk flow: key down → record → key up → transcribe → paste → save.

use crate::recorder::{Recorder, Recording};
use crate::{engine::Engine, hotkey, mascot, models, paste, settings::Settings, store::*};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Recordings shorter than this are treated as accidental taps.
const MIN_MS: i64 = 300;
/// Below this RMS the recording is considered silence.
const MIN_PEAK_RMS: f32 = 0.01;

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
    busy: AtomicBool,
    /// True while the app is recording a new hotkey: dictation is paused.
    pub capturing: AtomicBool,
    press: Mutex<Option<Press>>,
}

struct Press {
    started_at: i64,
    app_name: Option<String>,
}

impl Core {
    pub fn new(data_dir: PathBuf) -> anyhow::Result<Arc<Self>> {
        let settings = Settings::load(&data_dir);
        let store = Store::open(&data_dir.join("dicta.db"))?;
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
            busy: AtomicBool::new(false),
            capturing: AtomicBool::new(false),
            press: Mutex::new(None),
        }))
    }

    pub fn model_dir(&self) -> PathBuf {
        models::model_dir(&self.data_dir)
    }

    pub fn model_status(&self) -> ModelStatus {
        let mut s = self.model.lock().unwrap().clone();
        s.ready = models::is_ready(&self.model_dir());
        s.loaded = self.engine.is_loaded();
        s
    }
}

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
    if core.capturing.load(Ordering::SeqCst) {
        return;
    }
    if core.busy.swap(true, Ordering::SeqCst) {
        return; // still transcribing the previous one
    }
    if !models::is_ready(&core.model_dir()) {
        core.busy.store(false, Ordering::SeqCst);
        crate::open_panel(app, Some("settings"));
        return;
    }
    // Load the model while the user talks so release → paste is fast.
    let engine = core.engine.clone();
    std::thread::spawn(move || {
        if let Err(e) = engine.preload() {
            log::error!("preload: {e:#}");
        }
    });

    let mascot_on = core.settings.read().unwrap().mascot_enabled;
    let mic = core.settings.read().unwrap().mic.clone();
    *core.press.lock().unwrap() = Some(Press { started_at: now_ms(), app_name: paste::frontmost_app() });

    let level_app = app.clone();
    let on_level: crate::recorder::LevelFn = Arc::new(move |l| mascot::set_level(&level_app, l));
    match core.recorder.start(mic, on_level) {
        Ok(()) => {
            if mascot_on {
                mascot::show(app);
            }
        }
        Err(e) => {
            log::error!("recorder: {e:#}");
            core.busy.store(false, Ordering::SeqCst);
            if mascot_on {
                mascot::show(app);
                mascot::flash_then_swallow(app, "confused", 1200);
            }
            crate::open_panel(app, Some("settings"));
        }
    }
}

fn on_release(app: &AppHandle, core: Arc<Core>) {
    if !core.recorder.is_recording() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        finish(&app, &core);
        core.busy.store(false, Ordering::SeqCst);
    });
}

fn finish(app: &AppHandle, core: &Core) {
    let recording = match core.recorder.stop() {
        Ok(r) => r,
        Err(e) => {
            log::error!("stop: {e:#}");
            mascot::swallow(app);
            return;
        }
    };
    let press = core.press.lock().unwrap().take();
    let (started_at, app_name) = press.map(|p| (p.started_at, p.app_name)).unwrap_or((now_ms(), None));
    complete(app, core, recording, started_at, app_name);
}

fn complete(app: &AppHandle, core: &Core, recording: Recording, started_at: i64, app_name: Option<String>) {
    if recording.duration_ms < MIN_MS || recording.peak_rms < MIN_PEAK_RMS {
        log::info!("ignored: {} ms, peak {:.4}", recording.duration_ms, recording.peak_rms);
        mascot::swallow(app);
        return;
    }

    mascot::set_state(app, "thinking");
    let t = std::time::Instant::now();
    let result = core.engine.transcribe(&recording.samples);
    log::info!("transcribed {} ms of audio in {:?}", recording.duration_ms, t.elapsed());

    let new = match result {
        Ok(text) if text.is_empty() => {
            mascot::swallow(app);
            return;
        }
        Ok(raw) => {
            let rules = core.store.rules().unwrap_or_default();
            let text = crate::rules::apply(&raw, &rules);
            if let Err(e) = paste::paste_text(&text) {
                log::error!("paste: {e:#}");
            }
            mascot::set_state(app, "done");
            std::thread::sleep(Duration::from_millis(280));
            mascot::swallow(app);
            let language = detect_language(&text, &core.settings.read().unwrap().language);
            NewDictation { created_at: started_at, duration_ms: recording.duration_ms, text, language, app_name, error: None }
        }
        Err(e) => {
            log::error!("{e:#}");
            mascot::flash_then_swallow(app, "sad", 1200);
            NewDictation {
                created_at: started_at,
                duration_ms: recording.duration_ms,
                text: String::new(),
                language: None,
                app_name,
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
    let samples: Vec<f32> = match reader.spec().sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => reader.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?,
    };
    let started_at = now_ms();
    let app_name = paste::frontmost_app();
    let engine = core.engine.clone();
    std::thread::spawn(move || engine.preload());
    mascot::show(app);
    let chunk = 16_000 / 30;
    let mut peak = 0f32;
    for c in samples.chunks(chunk) {
        let rms = (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt();
        peak = peak.max(rms);
        mascot::set_level(app, (rms * 12.0).min(1.0));
        std::thread::sleep(Duration::from_millis(33));
    }
    let duration_ms = samples.len() as i64 * 1000 / 16_000;
    complete(app, &core, Recording { samples, duration_ms, peak_rms: peak }, started_at, app_name);
    Ok(())
}

/// The user's language setting wins; with "auto" we guess from the text.
fn detect_language(text: &str, setting: &str) -> Option<String> {
    if setting != "auto" {
        return Some(setting.to_string());
    }
    let info = whatlang::detect(text)?;
    // whatlang codes are ISO 639-3; map the common ones to ISO 639-1.
    let code = match info.lang().code() {
        "spa" => "es",
        "eng" => "en",
        "fra" => "fr",
        "deu" => "de",
        "por" => "pt",
        "ita" => "it",
        "nld" => "nl",
        "pol" => "pl",
        "ukr" => "uk",
        "rus" => "ru",
        other => other,
    };
    Some(code.to_string())
}

/// Periodically frees the model from RAM when idle.
pub fn spawn_idle_unloader(core: Arc<Core>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let min = core.settings.read().unwrap().idle_unload_min;
        if min > 0 && !core.busy.load(Ordering::SeqCst) {
            core.engine.unload_if_idle(Duration::from_secs(min as u64 * 60));
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
    }
}
