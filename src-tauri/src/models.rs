//! Locating and downloading the Parakeet v3 model (4 files from HuggingFace).
//! Files are pinned to one repo commit and checked by size and SHA-256.

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

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

/// Only one download may touch the `.part` files at a time.
static DOWNLOADING: Mutex<()> = Mutex::new(());

/// Streams `r` into `part`, hashing as it goes. Stops as soon as more than
/// `size` bytes arrive (a wrong/hostile server must not fill the disk) and
/// removes the partial file on any failure. `on_bytes` gets each chunk length.
fn copy_verified(
    mut r: impl Read,
    part: &Path,
    name: &str,
    size: u64,
    sha256: &str,
    mut on_bytes: impl FnMut(u64),
) -> Result<()> {
    let result = (|| -> Result<()> {
        let mut out = std::fs::File::create(part).with_context(|| format!("create {part:?}"))?;
        let mut hasher = Sha256::new();
        let mut written = 0u64;
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let n = r.read(&mut buf).with_context(|| format!("se cortó la descarga de {name}"))?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > size {
                bail!("{name} llegó más grande de lo esperado. Intenta de nuevo.");
            }
            out.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            on_bytes(n as u64);
        }
        out.sync_all()?;
        let hash = format!("{:x}", hasher.finalize());
        if written != size || !hash.eq_ignore_ascii_case(sha256) {
            bail!("{name} llegó incompleto o dañado ({written} de {size} bytes). Intenta de nuevo.");
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(part);
    }
    result
}

/// Downloads every file that is missing or has the wrong size, reporting
/// (bytes_done, bytes_total) across all of them.
pub fn download(dir: &Path, on_progress: impl Fn(u64, u64)) -> Result<()> {
    let _guard = match DOWNLOADING.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => bail!("ya se está descargando el modelo"),
    };
    std::fs::create_dir_all(dir)?;
    // No total timeout (the encoder is 650 MB) but a stalled connection must not hang forever.
    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .connect_timeout(Duration::from_secs(30))
        .user_agent("dilo")
        .build()?;

    let missing: Vec<&ModelFile> = FILES.iter().filter(|f| !file_ok(dir, f)).collect();
    let total: u64 = missing.iter().map(|f| f.size).sum();
    let mut done = 0u64;
    on_progress(done, total);

    for f in missing {
        let resp = client.get(format!("{BASE_URL}/{}", f.name)).send()?;
        if !resp.status().is_success() {
            bail!("download {}: HTTP {}", f.name, resp.status());
        }
        let part = dir.join(format!("{}.part", f.name));
        let mut last_report = done;
        copy_verified(resp, &part, f.name, f.size, f.sha256, |n| {
            done += n;
            if done - last_report > 1 << 20 {
                last_report = done;
                on_progress(done, total);
            }
        })?;
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

    // sha256("hello")
    const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    /// Yields `data` and then an I/O error, like a dropped connection.
    struct Flaky(&'static [u8], bool);
    impl Read for Flaky {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.0.is_empty() {
                return if self.1 { Err(std::io::ErrorKind::ConnectionReset.into()) } else { Ok(0) };
            }
            let n = self.0.len().min(buf.len());
            buf[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    #[test]
    fn copy_verified_accepts_exact_content() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("f.part");
        let mut seen = 0;
        copy_verified(&b"hello"[..], &part, "f", 5, HELLO, |n| seen += n).unwrap();
        assert_eq!(seen, 5);
        assert_eq!(std::fs::read(&part).unwrap(), b"hello");
    }

    #[test]
    fn copy_verified_rejects_and_cleans_bad_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("f.part");
        // truncated, corrupted (same size), oversized, connection reset
        assert!(copy_verified(&b"hell"[..], &part, "f", 5, HELLO, |_| {}).is_err());
        assert!(!part.exists());
        assert!(copy_verified(&b"hellp"[..], &part, "f", 5, HELLO, |_| {}).is_err());
        assert!(!part.exists());
        assert!(copy_verified(&b"hello!"[..], &part, "f", 5, HELLO, |_| {}).is_err());
        assert!(!part.exists());
        assert!(copy_verified(Flaky(b"hel", true), &part, "f", 5, HELLO, |_| {}).is_err());
        assert!(!part.exists());
        assert!(copy_verified(&b""[..], &part, "f", 5, HELLO, |_| {}).is_err());
    }

    #[test]
    fn copy_verified_overwrites_stale_part() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("f.part");
        std::fs::write(&part, vec![7u8; 1000]).unwrap();
        copy_verified(&b"hello"[..], &part, "f", 5, HELLO, |_| {}).unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), b"hello");
    }

    #[test]
    fn model_dir_is_under_models() {
        assert!(model_dir(Path::new("/x")).ends_with("models/parakeet-tdt-0.6b-v3-int8"));
    }
}
