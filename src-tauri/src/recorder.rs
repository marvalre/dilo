//! Microphone capture while the hotkey is held.
//!
//! cpal streams are not `Send` on macOS, so the stream lives on its own thread
//! and is dropped when `stop` signals it. Samples are downmixed to mono at the
//! device rate and resampled to 16 kHz once recording ends.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use rubato::{FftFixedIn, Resampler};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const TARGET_RATE: u32 = 16_000;
/// Level callbacks are throttled to about 30 per second.
const LEVEL_INTERVAL: Duration = Duration::from_millis(33);

pub struct Recording {
    pub samples: Vec<f32>,
    pub duration_ms: i64,
    pub peak_rms: f32,
}

struct Active {
    stop_tx: mpsc::Sender<()>,
    thread: JoinHandle<()>,
    buffer: Arc<Mutex<Vec<f32>>>,
    peak: Arc<Mutex<f32>>,
    rate: u32,
}

#[derive(Default)]
pub struct Recorder {
    active: Mutex<Option<Active>>,
}

pub type LevelFn = Arc<dyn Fn(f32) + Send + Sync>;

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
        let buffer = Arc::new(Mutex::new(Vec::<f32>::with_capacity(TARGET_RATE as usize * 30)));
        let peak = Arc::new(Mutex::new(0f32));
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32>>();

        let (buf, pk) = (buffer.clone(), peak.clone());
        let thread = std::thread::Builder::new().name("recorder".into()).spawn(move || {
            match open_stream(device_name, buf, pk, on_level) {
                Ok((stream, rate)) => {
                    let _ = ready_tx.send(Ok(rate));
                    let _ = stop_rx.recv(); // hold the stream until stop
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })?;

        let rate = ready_rx.recv().map_err(|_| anyhow!("recorder thread died"))??;
        *active = Some(Active { stop_tx, thread, buffer, peak, rate });
        Ok(())
    }

    pub fn stop(&self) -> Result<Recording> {
        let a = self.active.lock().unwrap().take().ok_or_else(|| anyhow!("not recording"))?;
        let _ = a.stop_tx.send(());
        let _ = a.thread.join();
        let raw = std::mem::take(&mut *a.buffer.lock().unwrap());
        let duration_ms = (raw.len() as f64 / a.rate as f64 * 1000.0) as i64;
        let samples = resample(&raw, a.rate, TARGET_RATE)?;
        let peak_rms = *a.peak.lock().unwrap();
        Ok(Recording { samples, duration_ms, peak_rms })
    }
}

fn open_stream(
    device_name: Option<String>,
    buffer: Arc<Mutex<Vec<f32>>>,
    peak: Arc<Mutex<f32>>,
    on_level: LevelFn,
) -> Result<(cpal::Stream, u32)> {
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
        SampleFormat::F32 => build::<f32>(&device, &stream_config, channels, buffer, peak, on_level)?,
        SampleFormat::I16 => build::<i16>(&device, &stream_config, channels, buffer, peak, on_level)?,
        SampleFormat::I32 => build::<i32>(&device, &stream_config, channels, buffer, peak, on_level)?,
        SampleFormat::U16 => build::<u16>(&device, &stream_config, channels, buffer, peak, on_level)?,
        other => return Err(anyhow!("unsupported sample format {other:?}")),
    };
    stream.play()?;
    Ok((stream, rate))
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    buffer: Arc<Mutex<Vec<f32>>>,
    peak: Arc<Mutex<f32>>,
    on_level: LevelFn,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: cpal::FromSample<T>,
{
    let mut last_level = Instant::now() - LEVEL_INTERVAL;
    let mut window_sq = 0f32;
    let mut window_n = 0usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mono = downmix(data, channels);
            for s in &mono {
                window_sq += s * s;
            }
            window_n += mono.len();
            buffer.lock().unwrap().extend_from_slice(&mono);

            if last_level.elapsed() >= LEVEL_INTERVAL && window_n > 0 {
                let rms = (window_sq / window_n as f32).sqrt();
                {
                    let mut p = peak.lock().unwrap();
                    *p = p.max(rms);
                }
                on_level((rms * 12.0).min(1.0));
                window_sq = 0.0;
                window_n = 0;
                last_level = Instant::now();
            }
        },
        |e| log::error!("input stream error: {e}"),
        None,
    )?;
    Ok(stream)
}

/// Interleaved frames of any sample type -> mono f32.
pub fn downmix<T>(data: &[T], channels: usize) -> Vec<f32>
where
    T: cpal::Sample,
    f32: cpal::FromSample<T>,
{
    let ch = channels.max(1);
    data.chunks(ch)
        .map(|frame| frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / ch as f32)
        .collect()
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
    fn downmix_averages_channels() {
        let stereo = [1.0f32, 0.0, 0.5, 0.5];
        assert_eq!(downmix(&stereo, 2), vec![0.5, 0.5]);
        let i16s = [i16::MAX, i16::MAX];
        assert!((downmix(&i16s, 1)[0] - 1.0).abs() < 1e-3);
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
