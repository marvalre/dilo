//! Speech-to-text engine: Parakeet v3 via transcribe-rs, loaded lazily and
//! unloaded after a period of inactivity so the app stays light on RAM.

use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity};
use transcribe_rs::onnx::Quantization;

pub struct Engine {
    dir: PathBuf,
    model: Mutex<Option<ParakeetModel>>,
    last_used: Mutex<Instant>,
}

impl Engine {
    pub fn new(dir: PathBuf) -> Arc<Self> {
        Arc::new(Self { dir, model: Mutex::new(None), last_used: Mutex::new(Instant::now()) })
    }

    pub fn is_loaded(&self) -> bool {
        self.model.lock().unwrap().is_some()
    }

    /// Loads the model if needed. Cheap when already loaded.
    pub fn preload(&self) -> Result<()> {
        let mut guard = self.model.lock().unwrap();
        if guard.is_none() {
            let t = Instant::now();
            let m = ParakeetModel::load(&self.dir, &Quantization::Int8)
                .map_err(|e| anyhow!("load Parakeet from {:?}: {e}", self.dir))?;
            log::info!("model loaded in {:?}", t.elapsed());
            *guard = Some(m);
        }
        *self.last_used.lock().unwrap() = Instant::now();
        Ok(())
    }

    /// `samples` must be mono f32 at 16 kHz.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        self.preload()?;
        let mut guard = self.model.lock().unwrap();
        let model = guard.as_mut().expect("preloaded");
        let params = ParakeetParams {
            timestamp_granularity: Some(TimestampGranularity::Segment),
            ..Default::default()
        };
        let text = model
            .transcribe_with(samples, &params)
            .map_err(|e| anyhow!("transcription failed: {e}"))?
            .text;
        *self.last_used.lock().unwrap() = Instant::now();
        Ok(text.trim().to_string())
    }

    /// Drops the model when idle longer than `idle`. Returns true if it unloaded.
    pub fn unload_if_idle(&self, idle: Duration) -> bool {
        if self.last_used.lock().unwrap().elapsed() < idle {
            return false;
        }
        // try_lock: never block on a transcription in progress.
        match self.model.try_lock() {
            Ok(mut g) if g.is_some() => {
                *g = None;
                log::info!("model unloaded after {:?} idle", idle);
                true
            }
            _ => false,
        }
    }
}
