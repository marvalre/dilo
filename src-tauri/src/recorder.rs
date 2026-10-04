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
    inner: Mutex<Inner>,
    peak: AtomicU32,
    truncated: AtomicBool,
    max_len: AtomicUsize,
}

/// Audio buffer plus the running level window, updated together under one lock
/// so the callback and the level thread never see a half-updated window.
#[derive(Default)]
struct Inner {
    buffer: Vec<f32>,
    /// Sum of squares / sample count for the current level window.
    window_sq: f64,
    window_n: usize,
}

impl Inner {
    /// Takes the window's RMS and resets it. None when no samples arrived.
    fn take_window_rms(&mut self) -> Option<f32> {
        let n = std::mem::take(&mut self.window_n);
        let sq = std::mem::take(&mut self.window_sq);
        (n > 0).then(|| (sq / n as f64).sqrt() as f32)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
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
        lock(&self.active).is_some()
    }

    /// Opens the input device and starts buffering. `device_name` None = default.
    pub fn start(&self, device_name: Option<String>, on_level: LevelFn) -> Result<()> {
        let mut active = lock(&self.active);
        if active.is_some() {
            return Ok(());
        }
        let shared = Arc::new(Shared {
            inner: Mutex::new(Inner { buffer: Vec::with_capacity(48_000 * 30), ..Default::default() }),
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
                                let rms = lock(&sh.inner).take_window_rms();
                                if let Some(rms) = rms {
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
        let a = lock(&self.active).take().ok_or_else(|| anyhow!("not recording"))?;
        let _ = a.stop_tx.send(());
        let _ = a.thread.join();
        let (raw, tail) = {
            let mut inner = lock(&a.shared.inner);
            // Short clips may end before the first level window closes.
            (std::mem::take(&mut inner.buffer), inner.take_window_rms().unwrap_or(0.0))
        };
        let duration_ms = (raw.len() as f64 / a.rate.max(1) as f64 * 1000.0) as i64;
        let samples = resample(&raw, a.rate, TARGET_RATE)?;
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
            let sq: f64 = mono.iter().map(|s| (*s as f64) * (*s as f64)).sum();
            let max = shared.max_len.load(Ordering::Relaxed);
            // Never panic on the real-time thread: a poisoned lock is still usable.
            let mut inner = lock(&shared.inner);
            inner.window_sq += sq;
            inner.window_n += mono.len();
            let room = max.saturating_sub(inner.buffer.len());
            if room < mono.len() {
                shared.truncated.store(true, Ordering::Relaxed);
            }
            inner.buffer.extend_from_slice(&mono[..room.min(mono.len())]);
        },
        |e| log::error!("input stream error: {e}"),
        None,
    )?;
    Ok(stream)
}

/// NaN/inf become silence and everything is kept inside [-1, 1]: a glitching
/// driver must not poison the level meter or feed garbage to the model.
fn clean(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
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
            return data.iter().map(|s| clean(s.to_sample::<f32>())).collect();
        }
        let frames = data.len() / ch;
        if frames == 0 {
            return Vec::new();
        }
        let mut chunk = vec![0f32; ch];
        for frame in data.chunks_exact(ch) {
            for (c, s) in frame.iter().enumerate() {
                let v = clean(s.to_sample::<f32>());
                chunk[c] += v * v;
            }
        }
        for (e, c) in self.energy.iter_mut().zip(&chunk) {
            *e = 0.8 * *e + 0.2 * (c / frames as f32);
        }
        let loudest = self.energy.iter().cloned().fold(0.0, f32::max);
        // Channels within ~12 dB of the loudest one.
        let mut live: Vec<usize> = if loudest <= 0.0 {
            (0..ch).collect()
        } else {
            (0..ch).filter(|&c| self.energy[c] >= loudest / 16.0).collect()
        };
        if live.is_empty() {
            live = (0..ch).collect();
        }
        data.chunks_exact(ch)
            .map(|frame| {
                clean(live.iter().map(|&c| clean(frame[c].to_sample::<f32>())).sum::<f32>() / live.len() as f32)
            })
            .collect()
    }
}

/// Mono resample. Output length is trimmed to the exact expected length.
pub fn resample(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    if from == to || input.is_empty() {
        return Ok(input.to_vec());
    }
    if from == 0 || to == 0 {
        return Err(anyhow!("invalid sample rate {from} -> {to}"));
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
    // The filter can overshoot a full-scale signal slightly.
    Ok(out[delay.min(end)..end].iter().map(|v| clean(*v)).collect())
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

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn resample_44k1_length_and_tone() {
        let sine: Vec<f32> =
            (0..44_100).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin()).collect();
        let out = resample(&sine, 44_100, 16_000).unwrap();
        assert_eq!(out.len(), 16_000);
        assert!((rms(&out[1000..15000]) - 0.707).abs() < 0.05);
    }

    #[test]
    fn resample_edge_lengths() {
        assert!(resample(&[], 48_000, 16_000).unwrap().is_empty());
        assert_eq!(resample(&[0.5; 7], 16_000, 16_000).unwrap(), vec![0.5; 7]);
        for len in [1usize, 2, 3, 5, 100, 1023, 1024, 1025, 2047, 3000] {
            for rate in [44_100u32, 48_000, 8_000, 96_000] {
                let input = vec![0.1f32; len];
                let out = resample(&input, rate, 16_000).unwrap();
                let expected = (len as f64 * 16_000.0 / rate as f64).round() as usize;
                assert_eq!(out.len(), expected, "len {len} rate {rate}");
                assert!(out.iter().all(|v| v.is_finite()));
            }
        }
        assert!(resample(&[0.0; 10], 0, 16_000).is_err());
    }

    #[test]
    fn resample_output_never_clips() {
        let square: Vec<f32> = (0..9600).map(|i| if (i / 50) % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let out = resample(&square, 48_000, 16_000).unwrap();
        assert!(out.iter().all(|v| v.abs() <= 1.0));
    }

    #[test]
    fn mixer_sanitizes_nan_inf_and_clipping() {
        let mut mono = Mixer::new(1);
        assert_eq!(mono.mix(&[f32::NAN, f32::INFINITY, -f32::INFINITY, 2.5, -3.0, 0.25]), vec![0.0, 0.0, 0.0, 1.0, -1.0, 0.25]);
        let mut st = Mixer::new(2);
        let out = st.mix(&[f32::NAN, f32::NAN, 0.5, 0.5, f32::NAN, 0.25]);
        assert!(out.iter().all(|v| v.is_finite()));
        assert_eq!(out.len(), 3);
        // Silent device: no division by zero.
        let mut silent = Mixer::new(4);
        assert!(silent.mix(&[0.0f32; 16]).iter().all(|v| *v == 0.0));
    }

    #[test]
    fn mixer_handles_partial_frames_and_zero_channels() {
        let mut m = Mixer::new(2);
        assert!(m.mix::<f32>(&[]).is_empty());
        assert!(m.mix(&[0.5f32]).is_empty()); // less than one frame
        assert_eq!(m.mix(&[0.5f32, 0.5, 0.9]).len(), 1); // trailing sample dropped
        let mut z = Mixer::new(0);
        assert_eq!(z.mix(&[0.1f32, 0.2]), vec![0.1, 0.2]);
    }

    #[test]
    fn level_window_rms_and_reset() {
        let mut w = Inner::default();
        assert_eq!(w.take_window_rms(), None);
        w.window_sq = 4.0 * 0.25;
        w.window_n = 4;
        let r = w.take_window_rms().unwrap();
        assert!((r - 0.5).abs() < 1e-6);
        assert_eq!(w.take_window_rms(), None);
    }

    #[test]
    fn lock_survives_poisoning() {
        let m = Arc::new(Mutex::new(1));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert_eq!(*lock(&m), 1);
    }
}
