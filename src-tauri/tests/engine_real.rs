//! Real-model test. Needs the Parakeet model downloaded (run the app once, or see README).
//! Run with: cargo test --release --test engine_real -- --ignored --nocapture

use dicta_lib::{engine::{Engine, LocalModel}, models};
use std::path::PathBuf;
use std::time::Instant;

fn model_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap();
    models::model_dir(&PathBuf::from(home).join("Library/Application Support/com.dicta.app"))
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_dicta"));
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
