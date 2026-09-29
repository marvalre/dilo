//! Speech-to-text engine: Parakeet v3 via transcribe-rs.
//!
//! The model runs in a separate worker process (this same binary started with
//! `--engine-worker <model_dir>`). Killing that process is the only way to give
//! all of the model's ~1 GB back to the OS — freeing it in-process leaves hundreds
//! of MB retained by the allocator. So the app itself stays at ~85 MB when idle.
//!
//! Protocol over the worker's stdin/stdout:
//!   worker → "ready\n" once the model is loaded (or "error <msg>\n")
//!   app → u32 LE sample count, then that many f32 LE samples (16 kHz mono)
//!   worker → one line: "ok <text>" or "error <msg>" (newlines in text escaped as \n)

use anyhow::{anyhow, bail, Context, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::mpsc;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity};
use transcribe_rs::onnx::Quantization;

pub const WORKER_FLAG: &str = "--engine-worker";

/// The model loaded in the current process. Used by the worker (and tests).
pub struct LocalModel(ParakeetModel);

impl LocalModel {
    pub fn load(dir: &Path) -> Result<Self> {
        let m = ParakeetModel::load(dir, &Quantization::Int8)
            .map_err(|e| anyhow!("load Parakeet from {dir:?}: {e}"))?;
        Ok(Self(m))
    }

    /// `samples` must be mono f32 at 16 kHz.
    pub fn transcribe(&mut self, samples: &[f32]) -> Result<String> {
        let params = ParakeetParams {
            timestamp_granularity: Some(TimestampGranularity::Segment),
            ..Default::default()
        };
        let r = self.0.transcribe_with(samples, &params).map_err(|e| anyhow!("transcription failed: {e}"))?;
        Ok(r.text.trim().to_string())
    }
}

/// Entry point of the worker process. Never returns.
pub fn worker_main(dir: &Path) -> ! {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut model = match LocalModel::load(dir) {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(out, "error {}", escape(&format!("{e:#}")));
            std::process::exit(1);
        }
    };
    let _ = writeln!(out, "ready");
    let _ = out.flush();

    let mut input = std::io::stdin().lock();
    loop {
        let samples = match read_samples(&mut input) {
            Ok(s) => s,
            Err(_) => std::process::exit(0), // parent closed the pipe
        };
        let line = match model.transcribe(&samples) {
            Ok(text) => format!("ok {}", escape(&text)),
            Err(e) => format!("error {}", escape(&format!("{e:#}"))),
        };
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn write_samples(w: &mut impl Write, samples: &[f32]) -> std::io::Result<()> {
    w.write_all(&(samples.len() as u32).to_le_bytes())?;
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    w.write_all(&bytes)?;
    w.flush()
}

fn read_samples(r: &mut impl Read) -> std::io::Result<Vec<f32>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let n = u32::from_le_bytes(len) as usize;
    let mut bytes = vec![0u8; n * 4];
    r.read_exact(&mut bytes)?;
    Ok(bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

/// Longest time to wait for the model to load.
const LOAD_TIMEOUT: Duration = Duration::from_secs(90);

struct Worker {
    child: Child,
    /// Audio to send, written on a helper thread: a stuck worker stops reading
    /// its stdin and a blocking write would never return.
    audio: mpsc::Sender<Vec<f32>>,
    /// Lines from the worker's stdout, read on a helper thread so waits can time out.
    lines: mpsc::Receiver<String>,
}

impl Worker {
    fn spawn(mut child: Child) -> Self {
        let mut stdin = child.stdin.take().unwrap();
        let (audio, audio_rx) = mpsc::channel::<Vec<f32>>();
        std::thread::Builder::new()
            .name("engine-writer".into())
            .spawn(move || {
                for samples in audio_rx {
                    if write_samples(&mut stdin, &samples).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn engine writer");
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (tx, lines) = mpsc::channel();
        std::thread::Builder::new()
            .name("engine-reader".into())
            .spawn(move || {
                for line in stdout.lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn engine reader");
        Self { child, audio, lines }
    }

    fn read_line(&mut self, timeout: Duration) -> Result<String> {
        match self.lines.recv_timeout(timeout) {
            Ok(line) => Ok(line),
            Err(mpsc::RecvTimeoutError::Timeout) => bail!("el motor tardó demasiado (más de {} s)", timeout.as_secs()),
            Err(mpsc::RecvTimeoutError::Disconnected) => bail!("engine worker exited"),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Manages the worker process: started on demand, killed when idle.
pub struct Engine {
    dir: PathBuf,
    exe: PathBuf,
    worker: Mutex<Option<Worker>>,
    last_used: Mutex<Instant>,
}

impl Engine {
    /// `exe` is the binary that understands `--engine-worker` (normally `current_exe()`).
    pub fn new(dir: PathBuf, exe: PathBuf) -> Arc<Self> {
        Arc::new(Self { dir, exe, worker: Mutex::new(None), last_used: Mutex::new(Instant::now()) })
    }

    pub fn is_loaded(&self) -> bool {
        self.worker.try_lock().map(|w| w.is_some()).unwrap_or(true)
    }

    /// Starts the worker and waits until the model is loaded. Cheap when already running.
    pub fn preload(&self) -> Result<()> {
        let mut guard = self.worker.lock().unwrap();
        self.ensure(&mut guard)?;
        *self.last_used.lock().unwrap() = Instant::now();
        Ok(())
    }

    fn ensure(&self, guard: &mut Option<Worker>) -> Result<()> {
        if let Some(w) = guard.as_mut() {
            if w.child.try_wait()?.is_none() {
                return Ok(());
            }
            *guard = None; // it died; start a fresh one
        }
        let t = Instant::now();
        let child = Command::new(&self.exe)
            .arg(WORKER_FLAG)
            .arg(&self.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("start engine worker {:?}", self.exe))?;
        let mut w = Worker::spawn(child);
        let first = w.read_line(LOAD_TIMEOUT)?; // on error `w` drops and the process is killed
        if first != "ready" {
            bail!("engine worker: {}", unescape(first.trim_start_matches("error ")));
        }
        log::info!("engine worker ready in {:?}", t.elapsed());
        *guard = Some(w);
        Ok(())
    }

    /// `samples` must be mono f32 at 16 kHz.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        let mut guard = self.worker.lock().unwrap();
        self.ensure(&mut guard)?;
        let w = guard.as_mut().unwrap();
        // Parakeet runs ~20× faster than real time; allow a generous margin.
        let timeout = Duration::from_secs(30) + Duration::from_secs_f32(samples.len() as f32 / 16_000.0);
        let result = w
            .audio
            .send(samples.to_vec())
            .map_err(|_| anyhow!("engine worker exited"))
            .and_then(|_| w.read_line(timeout));
        *self.last_used.lock().unwrap() = Instant::now();
        let line = match result {
            Ok(l) => l,
            Err(e) => {
                *guard = None; // hung or broken: kill it, restart next time
                return Err(e);
            }
        };
        if let Some(text) = line.strip_prefix("ok ") {
            Ok(unescape(text))
        } else if line == "ok" {
            Ok(String::new())
        } else {
            bail!("{}", unescape(line.trim_start_matches("error ")))
        }
    }

    /// Kills the worker when idle longer than `idle`. Returns true if it did.
    pub fn unload_if_idle(&self, idle: Duration) -> bool {
        if self.last_used.lock().unwrap().elapsed() < idle {
            return false;
        }
        // try_lock: never block on a transcription in progress.
        match self.worker.try_lock() {
            Ok(mut g) if g.is_some() => {
                *g = None; // Drop kills the process
                log::info!("engine worker stopped after {idle:?} idle");
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_roundtrip() {
        for s in ["hola\nmundo", "a\\nb", "plain", "trailing\\"] {
            assert_eq!(unescape(&escape(s)), s);
        }
    }

    #[test]
    fn samples_roundtrip() {
        let s = vec![0.0f32, -1.0, 0.5, 1e-7];
        let mut buf = Vec::new();
        write_samples(&mut buf, &s).unwrap();
        assert_eq!(read_samples(&mut buf.as_slice()).unwrap(), s);
    }
}
