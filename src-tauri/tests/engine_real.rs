//! Real-model test. Needs the Parakeet model downloaded (run the app once, or see README).
//! Run with: cargo test --release --test engine_real -- --ignored --nocapture

use dicta_lib::{engine::Engine, models};
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
    let engine = Engine::new(model_dir());
    let t = Instant::now();
    engine.preload().unwrap();
    println!("load: {:?}", t.elapsed());

    for (file, must_contain) in [("es.wav", "prueba"), ("en.wav", "report")] {
        let samples = read_wav(file);
        let t = Instant::now();
        let text = engine.transcribe(&samples).unwrap();
        println!("{file} ({:.1}s audio) in {:?}: {text}", samples.len() as f32 / 16000.0, t.elapsed());
        assert!(text.to_lowercase().contains(must_contain), "{text}");
    }
}
