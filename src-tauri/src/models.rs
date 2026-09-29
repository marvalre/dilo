//! Locating and downloading the Parakeet v3 model (4 files from HuggingFace).
//! Files are pinned to one repo commit and checked by size and SHA-256.

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MODEL_ID: &str = "parakeet-tdt-0.6b-v3-int8";
const BASE_URL: &str =
    "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce";

pub struct ModelFile {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub const FILES: [ModelFile; 4] = [
    ModelFile { name: "vocab.txt", size: 93_939, sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d" },
    ModelFile { name: "nemo128.onnx", size: 139_764, sha256: "a9fde1486ebfcc08f328d75ad4610c67835fea58c73ba57e3209a6f6cf019e9f" },
    ModelFile {
        name: "decoder_joint-model.int8.onnx",
        size: 18_202_004,
        sha256: "eea7483ee3d1a30375daedc8ed83e3960c91b098812127a0d99d1c8977667a70",
    },
    ModelFile {
        name: "encoder-model.int8.onnx",
        size: 652_183_999,
        sha256: "6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09",
    },
];

pub fn model_dir(app_data: &Path) -> PathBuf {
    app_data.join("models").join(MODEL_ID)
}

fn file_ok(dir: &Path, f: &ModelFile) -> bool {
    std::fs::metadata(dir.join(f.name)).map(|m| m.len() == f.size).unwrap_or(false)
}

/// Cheap check (exact sizes). Hashes are verified when a file is downloaded.
pub fn is_ready(dir: &Path) -> bool {
    FILES.iter().all(|f| file_ok(dir, f))
}

/// Downloads every file that is missing or has the wrong size, reporting
/// (bytes_done, bytes_total) across all of them.
pub fn download(dir: &Path, on_progress: impl Fn(u64, u64)) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let client = reqwest::blocking::Client::builder().timeout(None).user_agent("dilo").build()?;

    let missing: Vec<&ModelFile> = FILES.iter().filter(|f| !file_ok(dir, f)).collect();
    let total: u64 = missing.iter().map(|f| f.size).sum();
    let mut done = 0u64;
    on_progress(done, total);

    for f in missing {
        let mut resp = client.get(format!("{BASE_URL}/{}", f.name)).send()?;
        if !resp.status().is_success() {
            bail!("download {}: HTTP {}", f.name, resp.status());
        }
        let part = dir.join(format!("{}.part", f.name));
        let mut out = std::fs::File::create(&part).with_context(|| format!("create {part:?}"))?;
        let mut hasher = Sha256::new();
        let mut written = 0u64;
        let mut buf = vec![0u8; 1 << 16];
        let mut last_report = 0u64;
        loop {
            let n = resp.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            written += n as u64;
            done += n as u64;
            if done - last_report > 1 << 20 {
                last_report = done;
                on_progress(done, total);
            }
        }
        out.flush()?;
        drop(out);
        let hash = format!("{:x}", hasher.finalize());
        if written != f.size || hash != f.sha256 {
            let _ = std::fs::remove_file(&part);
            bail!("{} llegó incompleto o dañado ({written} de {} bytes). Intenta de nuevo.", f.name, f.size);
        }
        std::fs::rename(&part, dir.join(f.name))?;
    }
    on_progress(total, total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_only_when_every_file_has_its_exact_size() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_ready(dir.path()));
        for f in &FILES {
            let file = std::fs::File::create(dir.path().join(f.name)).unwrap();
            file.set_len(f.size).unwrap();
        }
        assert!(is_ready(dir.path()));
        std::fs::File::create(dir.path().join(FILES[3].name)).unwrap().set_len(10).unwrap();
        assert!(!is_ready(dir.path()));
    }
}
