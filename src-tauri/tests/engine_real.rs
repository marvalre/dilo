//! Real-model test. Needs the Parakeet model downloaded (run the app once, or see README).
//! Run with: cargo test --release --test engine_real -- --ignored --nocapture

use dilo_lib::{engine::{Engine, LocalModel}, models};
use std::path::PathBuf;
use std::time::Instant;

fn model_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap();
    models::model_dir(&PathBuf::from(home).join("Library/Application Support/com.dilo.app"))
}

fn read_wav(name: &str) -> Vec<f32> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    let mut r = hound::WavReader::open(path).unwrap();
    assert_eq!(r.spec().sample_rate, 16_000);
    r.samples::<f32>().map(|s| s.unwrap()).collect()
}

#[test]
#[ignore]
fn transcribes_spanish_and_english() {
    let mut engine = LocalModel::load(&model_dir()).unwrap();
    let t = Instant::now();
    println!("load: {:?}", t.elapsed());

    for (file, must_contain) in [("es.wav", "prueba"), ("en.wav", "report")] {
        let samples = read_wav(file);
        let t = Instant::now();
        let text = engine.transcribe(&samples).unwrap();
        println!("{file} ({:.1}s audio) in {:?}: {text}", samples.len() as f32 / 16000.0, t.elapsed());
        assert!(text.to_lowercase().contains(must_contain), "{text}");
    }
}

fn rss_mb() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap() / 1024
}

#[test]
#[ignore]
fn worker_process_transcribes_and_stops() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_dilo"));
    let engine = Engine::new(model_dir(), exe);
    let t = Instant::now();
    engine.preload().unwrap();
    println!("worker ready in {:?}, app RSS {} MB", t.elapsed(), rss_mb());
    let t = Instant::now();
    let text = engine.transcribe(&read_wav("es.wav")).unwrap();
    println!("worker transcribed in {:?}: {text}", t.elapsed());
    assert!(text.to_lowercase().contains("prueba"));
    assert!(engine.unload_if_idle(std::time::Duration::ZERO));
    assert!(!engine.is_loaded());
}

#[test]
#[ignore]
fn long_audio_keeps_start_and_end() {
    let mut engine = LocalModel::load(&model_dir()).unwrap();
    let samples = read_wav("long_es.wav");
    let long: Vec<f32> = samples.iter().chain(samples.iter()).copied().collect();
    for (label, audio) in [("27s", samples.clone()), ("54s", long), ("27s quiet", samples.iter().map(|s| s * 0.05).collect())] {
        let text = engine.transcribe(&audio).unwrap();
        println!("{label}: {text}\n");
        let t = text.to_lowercase();
        assert!(t.starts_with("primero"), "{label} lost the start: {text}");
        assert!(t.contains("terminar hoy"), "{label} lost the end: {text}");
    }
}

#[test]
#[ignore]
fn mic_rate_44k_resampled_matches() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/long_es_44k.wav");
    let mut r = hound::WavReader::open(path).unwrap();
    let raw: Vec<f32> = r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
    let samples = dilo_lib::recorder::resample(&raw, 44_100, 16_000).unwrap();
    let mut engine = LocalModel::load(&model_dir()).unwrap();
    let text = engine.transcribe(&samples).unwrap();
    println!("44k→16k: {text}");
    let t = text.to_lowercase();
    assert!(t.starts_with("primero") && t.contains("terminar hoy") && t.contains("subagentes"), "{text}");
}
