//! Microphone capture while the hotkey is held.
//!
//! cpal streams are not `Send` on macOS, so the stream lives on its own thread
//! and is dropped when `stop` signals it. Samples are downmixed to mono at the
//! device rate and resampled to 16 kHz once recording ends.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use rubato::{FftFixedIn, Resampler};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

pub const TARGET_RATE: u32 = 16_000;
/// Level updates for the mascot, about 30 per second.
const LEVEL_INTERVAL: Duration = Duration::from_millis(33);
/// Hard cap on a single dictation; audio beyond this is dropped.
pub const MAX_SECONDS: u32 = 300;

pub struct Recording {
    pub samples: Vec<f32>,
    pub duration_ms: i64,
    pub peak_rms: f32,
    pub truncated: bool,
}

/// Shared between the real-time audio callback and the rest of the app.
/// The callback only does arithmetic, one uncontended lock and atomic stores.
struct Shared {
    buffer: Mutex<Vec<f32>>,
    /// Running sum of squares / count for the current level window (f32 bits).
    window_sq: AtomicU32,
    window_n: AtomicUsize,
    peak: AtomicU32,
    truncated: AtomicBool,
    max_len: AtomicUsize,
}

struct Active {
    stop_tx: mpsc::Sender<()>,
    thread: JoinHandle<()>,
    shared: Arc<Shared>,
    rate: u32,
}

#[derive(Default)]
pub struct Recorder {
    active: Mutex<Option<Active>>,
}

pub type LevelFn = Arc<dyn Fn(f32) + Send + Sync>;

fn load_f32(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}

impl Recorder {
    pub fn is_recording(&self) -> bool {
        self.active.lock().unwrap().is_some()
    }

    /// Opens the input device and starts buffering. `device_name` None = default.
    pub fn start(&self, device_name: Option<String>, on_level: LevelFn) -> Result<()> {
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            return Ok(());
        }
        let shared = Arc::new(Shared {
            buffer: Mutex::new(Vec::with_capacity(48_000 * 30)),
            window_sq: AtomicU32::new(0),
            window_n: AtomicUsize::new(0),
            peak: AtomicU32::new(0),
            truncated: AtomicBool::new(false),
            max_len: AtomicUsize::new(usize::MAX),
        });
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32>>();

        let sh = shared.clone();
        let thread = std::thread::Builder::new().name("recorder".into()).spawn(move || {
            match open_stream(device_name, sh.clone()) {
                Ok((stream, rate)) => {
                    let _ = ready_tx.send(Ok(rate));
                    // Hold the stream until stop; meanwhile publish the voice level.
                    loop {
                        match stop_rx.recv_timeout(LEVEL_INTERVAL) {
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                let n = sh.window_n.swap(0, Ordering::Relaxed);
                                let sq = f32::from_bits(sh.window_sq.swap(0, Ordering::Relaxed));
                                if n > 0 {
                                    let rms = (sq / n as f32).sqrt();
                                    if rms > load_f32(&sh.peak) {
                                        sh.peak.store(rms.to_bits(), Ordering::Relaxed);
                                    }
                                    on_level((rms * 12.0).min(1.0));
                                }
                            }
                            _ => break,
                        }
                    }
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })?;

        let rate = ready_rx.recv().map_err(|_| anyhow!("recorder thread died"))??;
        shared.max_len.store(rate as usize * MAX_SECONDS as usize, Ordering::Relaxed);
        *active = Some(Active { stop_tx, thread, shared, rate });
        Ok(())
    }

    pub fn stop(&self) -> Result<Recording> {
        let a = self.active.lock().unwrap().take().ok_or_else(|| anyhow!("not recording"))?;
        let _ = a.stop_tx.send(());
        let _ = a.thread.join();
        let raw = std::mem::take(&mut *a.shared.buffer.lock().unwrap());
        let duration_ms = (raw.len() as f64 / a.rate as f64 * 1000.0) as i64;
        let samples = resample(&raw, a.rate, TARGET_RATE)?;
        // Short clips may end before the first level window closes.
        let n = a.shared.window_n.load(Ordering::Relaxed);
        let tail = if n > 0 { (load_f32(&a.shared.window_sq) / n as f32).sqrt() } else { 0.0 };
        let peak_rms = load_f32(&a.shared.peak).max(tail);
        Ok(Recording { samples, duration_ms, peak_rms, truncated: a.shared.truncated.load(Ordering::Relaxed) })
    }
}

fn open_stream(device_name: Option<String>, shared: Arc<Shared>) -> Result<(cpal::Stream, u32)> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .input_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .or_else(|| host.default_input_device()),
        None => host.default_input_device(),
    }
    .ok_or_else(|| anyhow!("no microphone found"))?;

    let config = device.default_input_config()?;
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &stream_config, channels, shared)?,
        SampleFormat::I16 => build::<i16>(&device, &stream_config, channels, shared)?,
        SampleFormat::I32 => build::<i32>(&device, &stream_config, channels, shared)?,
        SampleFormat::U16 => build::<u16>(&device, &stream_config, channels, shared)?,
        other => return Err(anyhow!("unsupported sample format {other:?}")),
    };
    stream.play()?;
    Ok((stream, rate))
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, channels: usize, shared: Arc<Shared>) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: cpal::FromSample<T>,
{
    let mut mixer = Mixer::new(channels);
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mono = mixer.mix(data);
            let sq: f32 = mono.iter().map(|s| s * s).sum();
            let prev = f32::from_bits(shared.window_sq.load(Ordering::Relaxed));
            shared.window_sq.store((prev + sq).to_bits(), Ordering::Relaxed);
            shared.window_n.fetch_add(mono.len(), Ordering::Relaxed);

            let max = shared.max_len.load(Ordering::Relaxed);
            let mut buf = shared.buffer.lock().unwrap();
            let room = max.saturating_sub(buf.len());
            if room < mono.len() {
                shared.truncated.store(true, Ordering::Relaxed);
            }
            buf.extend_from_slice(&mono[..room.min(mono.len())]);
        },
        |e| log::error!("input stream error: {e}"),
        None,
    )?;
    Ok(stream)
}

/// Downmix that follows the channels actually carrying sound: averaging every
/// channel of an 8-input interface would bury a mic plugged into input 1.
pub struct Mixer {
    energy: Vec<f32>,
}

impl Mixer {
    pub fn new(channels: usize) -> Self {
        Self { energy: vec![0.0; channels.max(1)] }
    }

    pub fn mix<T>(&mut self, data: &[T]) -> Vec<f32>
    where
        T: cpal::Sample,
        f32: cpal::FromSample<T>,
    {
        let ch = self.energy.len();
        if ch == 1 {
            return data.iter().map(|s| s.to_sample::<f32>()).collect();
        }
        let frames = data.len() / ch;
        if frames == 0 {
            return Vec::new();
        }
        let mut chunk = vec![0f32; ch];
        for frame in data.chunks_exact(ch) {
            for (c, s) in frame.iter().enumerate() {
                let v = s.to_sample::<f32>();
                chunk[c] += v * v;
            }
        }
        for (e, c) in self.energy.iter_mut().zip(&chunk) {
            *e = 0.8 * *e + 0.2 * (c / frames as f32);
        }
        let loudest = self.energy.iter().cloned().fold(0.0, f32::max);
        // Channels within ~12 dB of the loudest one.
        let live: Vec<usize> = if loudest <= 0.0 {
            (0..ch).collect()
        } else {
            (0..ch).filter(|&c| self.energy[c] >= loudest / 16.0).collect()
        };
        data.chunks_exact(ch)
            .map(|frame| live.iter().map(|&c| frame[c].to_sample::<f32>()).sum::<f32>() / live.len() as f32)
            .collect()
    }
}

/// Mono resample. Output length is trimmed to the exact expected length.
pub fn resample(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    if from == to || input.is_empty() {
        return Ok(input.to_vec());
    }
    const CHUNK: usize = 1024;
    let mut rs = FftFixedIn::<f32>::new(from as usize, to as usize, CHUNK, 2, 1)?;
    let delay = rs.output_delay();
    let expected = (input.len() as f64 * to as f64 / from as f64).round() as usize;
    let mut out = Vec::with_capacity(expected + delay + CHUNK);

    let mut chunks = input.chunks_exact(CHUNK);
    for chunk in &mut chunks {
        out.extend_from_slice(&rs.process(&[chunk], None)?[0]);
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        out.extend_from_slice(&rs.process_partial(Some(&[rest]), None)?[0]);
    }
    // Flush the resampler's internal delay.
    while out.len() < expected + delay {
        let flushed = rs.process_partial::<&[f32]>(None, None)?;
        if flushed[0].is_empty() {
            break;
        }
        out.extend_from_slice(&flushed[0]);
    }
    let end = (delay + expected).min(out.len());
    Ok(out[delay.min(end)..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_follows_the_live_channel() {
        let mut m = Mixer::new(8);
        // Voice on channel 0 only, the other 7 silent.
        let frames: Vec<f32> = (0..4800).flat_map(|i| {
            let v = (i as f32 * 0.05).sin() * 0.3;
            std::iter::once(v).chain(std::iter::repeat(0.0).take(7))
        }).collect();
        let out = m.mix(&frames);
        let rms = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt();
        assert!(rms > 0.18, "voice kept at full level, rms {rms}");

        let mut stereo = Mixer::new(2);
        assert_eq!(stereo.mix(&[0.5f32, 0.5, 0.2, 0.2]), vec![0.5, 0.2]);
    }

    #[test]
    fn resample_48k_to_16k_keeps_length_and_tone() {
        let sine: Vec<f32> =
            (0..48_000).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin()).collect();
        let out = resample(&sine, 48_000, 16_000).unwrap();
        assert_eq!(out.len(), 16_000);
        let rms = (out[1000..15000].iter().map(|s| s * s).sum::<f32>() / 14000.0).sqrt();
        assert!((rms - 0.707).abs() < 0.05, "rms {rms}");
    }
}
