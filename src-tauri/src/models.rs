//! Locating and downloading the Parakeet v3 model (4 files from HuggingFace).

use anyhow::{bail, Context, Result};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MODEL_ID: &str = "parakeet-tdt-0.6b-v3-int8";
const BASE_URL: &str = "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main";
pub const FILES: [&str; 4] =
    ["vocab.txt", "nemo128.onnx", "decoder_joint-model.int8.onnx", "encoder-model.int8.onnx"];

pub fn model_dir(app_data: &Path) -> PathBuf {
    app_data.join("models").join(MODEL_ID)
}

pub fn is_ready(dir: &Path) -> bool {
    FILES
        .iter()
        .all(|f| std::fs::metadata(dir.join(f)).map(|m| m.len() > 0).unwrap_or(false))
}

/// Downloads missing files, reporting (bytes_done, bytes_total) across all files.
pub fn download(dir: &Path, on_progress: impl Fn(u64, u64)) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .user_agent("dicta")
        .build()?;

    let missing: Vec<&str> = FILES.iter().copied().filter(|f| !dir.join(f).exists()).collect();
    let mut sizes = Vec::new();
    for f in &missing {
        let r = client.head(format!("{BASE_URL}/{f}")).send()?;
        sizes.push(r.content_length().unwrap_or(0));
    }
    let total: u64 = sizes.iter().sum();
    let mut done = 0u64;
    on_progress(done, total);

    for f in missing {
        let mut resp = client.get(format!("{BASE_URL}/{f}")).send()?;
        if !resp.status().is_success() {
            bail!("download {f}: HTTP {}", resp.status());
        }
        let part = dir.join(format!("{f}.part"));
        let mut out = std::fs::File::create(&part).with_context(|| format!("create {part:?}"))?;
        let mut buf = vec![0u8; 1 << 16];
        let mut last_report = 0u64;
        loop {
            let n = resp.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            if done - last_report > 1 << 20 {
                last_report = done;
                on_progress(done, total);
            }
        }
        out.flush()?;
        std::fs::rename(&part, dir.join(f))?;
    }
    on_progress(total, total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_only_when_all_files_present_and_non_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_ready(dir.path()));
        for f in FILES {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        assert!(is_ready(dir.path()));
        std::fs::write(dir.path().join(FILES[0]), b"").unwrap();
        assert!(!is_ready(dir.path()));
    }
}
