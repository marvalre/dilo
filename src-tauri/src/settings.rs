//! User settings persisted as JSON in the app data dir.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// handy-keys hotkey string, e.g. "Fn", "CtrlRight", "Ctrl+Space".
    pub hotkey: String,
    /// "auto" or an ISO 639-1 code; Spanish by default. Parakeet cannot be forced to a language,
    /// so this labels history and stats.
    pub language: String,
    /// Free the model from RAM after this many idle minutes (0 = never).
    pub idle_unload_min: u32,
    pub mascot_enabled: bool,
    /// "wave" | "glass" | "jolly" | "dot" | "aura"
    pub mascot_skin: String,
    /// "s" | "m" | "l"
    pub mascot_size: String,
    /// Input device name; None = system default.
    pub mic: Option<String>,
    pub launch_at_login: bool,
    /// Warn in the app when total storage (model + history) exceeds this.
    pub storage_warn_mb: u64,
    /// Look for new versions in the background.
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey().into(),
            language: "es".into(),
            idle_unload_min: 2,
            mascot_enabled: true,
            mascot_skin: "wave".into(),
            mascot_size: "m".into(),
            mic: None,
            launch_at_login: false,
            storage_warn_mb: 2000,
            auto_update: true,
        }
    }
}

pub fn default_hotkey() -> &'static str {
    if cfg!(target_os = "macos") {
        "Fn"
    } else {
        "CtrlRight"
    }
}

pub const SKINS: [&str; 5] = ["wave", "glass", "jolly", "dot", "aura"];
pub const SIZES: [&str; 3] = ["s", "m", "l"];

/// Upper bounds so absurd values (hand-edited file, buggy UI) can't overflow later arithmetic.
pub const MAX_IDLE_UNLOAD_MIN: u32 = 7 * 24 * 60;
pub const MAX_STORAGE_WARN_MB: u64 = 100_000_000;

impl Settings {
    /// Fixes out-of-range values coming from the UI or an old settings file.
    pub fn sanitized(mut self) -> Self {
        if !SKINS.contains(&self.mascot_skin.as_str()) {
            self.mascot_skin = "wave".into();
        }
        if !SIZES.contains(&self.mascot_size.as_str()) {
            self.mascot_size = "m".into();
        }
        if crate::hotkey::validate(&self.hotkey).is_err() {
            self.hotkey = default_hotkey().into();
        }
        let lang = self.language.trim().to_string();
        let lang_ok = !lang.is_empty()
            && lang.len() <= 12
            && lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        self.language = if lang_ok { lang } else { "es".into() };
        self.idle_unload_min = self.idle_unload_min.min(MAX_IDLE_UNLOAD_MIN);
        self.storage_warn_mb = self.storage_warn_mb.min(MAX_STORAGE_WARN_MB);
        // An empty device name means "system default", not a device called "".
        self.mic = self.mic.take().filter(|m| !m.trim().is_empty());
        self
    }

    /// Warning threshold in bytes (saturating, never overflows).
    pub fn storage_warn_bytes(&self) -> u64 {
        self.storage_warn_mb.saturating_mul(1_000_000)
    }

    /// Parses settings JSON field by field: a wrong type in one field (`"idle_unload_min": "x"`)
    /// only resets that field instead of throwing away the whole file.
    pub fn from_json_lenient(raw: &str) -> Option<Settings> {
        let value: serde_json::Value = serde_json::from_str(raw).ok()?;
        let file = value.as_object()?;
        let mut merged = match serde_json::to_value(Settings::default()) {
            Ok(serde_json::Value::Object(m)) => m,
            _ => return None,
        };
        for (key, val) in file {
            if !merged.contains_key(key) {
                continue; // unknown field
            }
            let previous = merged.insert(key.clone(), val.clone());
            if serde_json::from_value::<Settings>(serde_json::Value::Object(merged.clone())).is_err() {
                match previous {
                    Some(p) => merged.insert(key.clone(), p),
                    None => merged.remove(key),
                };
            }
        }
        serde_json::from_value(serde_json::Value::Object(merged)).ok()
    }

    pub fn load(dir: &Path) -> Settings {
        let path = dir.join(FILE);
        let raw = match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(_) => return Settings::default().sanitized(),
        };
        // A BOM (some Windows editors) would make serde_json reject an otherwise valid file.
        let parsed = Self::from_json_lenient(raw.trim_start_matches('\u{feff}'));
        if parsed.is_none() {
            // Keep the broken file for inspection; the next save would overwrite it.
            let _ = std::fs::copy(&path, dir.join(format!("{FILE}.corrupt")));
            log::warn!("settings.json is not valid JSON; using defaults");
        }
        parsed.unwrap_or_default().sanitized()
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        use std::io::Write;
        std::fs::create_dir_all(dir)?;
        // Write, flush to disk, then rename, so a crash mid-write never leaves a broken file.
        let tmp = dir.join(format!("{FILE}.tmp"));
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, dir.join(FILE))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults_and_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
        let s = Settings { language: "es".into(), idle_unload_min: 3, ..Default::default() };
        s.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()), s);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), r#"{"language":"en"}"#).unwrap();
        let s = Settings::load(dir.path());
        assert_eq!(s.language, "en");
        assert_eq!(s.idle_unload_min, 2);
        assert_eq!(Settings::default().language, "es");
        std::fs::write(dir.path().join(FILE), r#"{"language":"auto"}"#).unwrap();
        assert_eq!(Settings::load(dir.path()).language, "auto");
        std::fs::write(dir.path().join(FILE), r#"{"language":""}"#).unwrap();
        assert_eq!(Settings::load(dir.path()).language, "es");
    }

    #[test]
    fn invalid_hotkey_on_disk_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), r#"{"hotkey":"NotAKey+++"}"#).unwrap();
        assert_eq!(Settings::load(dir.path()).hotkey, default_hotkey());
    }

    #[test]
    fn one_wrong_field_does_not_reset_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            r#"{"language":"en","idle_unload_min":"lots","mascot_enabled":"yes","storage_warn_mb":-5,"mascot_size":"l","unknown":1}"#,
        )
        .unwrap();
        let s = Settings::load(dir.path());
        assert_eq!(s.language, "en");
        assert_eq!(s.mascot_size, "l");
        assert_eq!(s.idle_unload_min, 2);
        assert!(s.mascot_enabled);
        assert_eq!(s.storage_warn_mb, 2000);
    }

    #[test]
    fn corrupt_or_odd_files_fall_back_and_keep_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        for raw in ["", "{", "[]", "null", "42", "\u{0}\u{0}", "{\"language\": }"] {
            std::fs::write(dir.path().join(FILE), raw).unwrap();
            assert_eq!(Settings::load(dir.path()), Settings::default(), "raw={raw:?}");
        }
        assert!(dir.path().join("settings.json.corrupt").exists());
        std::fs::write(dir.path().join(FILE), [0xff, 0xfe, 0x00, 0x9f]).unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
        std::fs::write(dir.path().join(FILE), "\u{feff}{\"language\":\"fr\"}").unwrap();
        assert_eq!(Settings::load(dir.path()).language, "fr");
    }

    #[test]
    fn sanitize_clamps_extremes() {
        let s = Settings {
            idle_unload_min: u32::MAX,
            storage_warn_mb: u64::MAX,
            language: "  ../../etc  ".into(),
            mascot_skin: "x".into(),
            mic: Some("  ".into()),
            ..Default::default()
        }
        .sanitized();
        assert_eq!(s.idle_unload_min, MAX_IDLE_UNLOAD_MIN);
        assert_eq!(s.language, "es");
        assert_eq!(s.mascot_skin, "wave");
        assert_eq!(s.mic, None);
        assert!(s.storage_warn_bytes() > 0); // no overflow
        assert_eq!(Settings { language: " pt-BR ".into(), ..Default::default() }.sanitized().language, "pt-BR");
    }

    #[test]
    fn save_is_atomic_and_leaves_no_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        Settings::default().save(&nested).unwrap();
        Settings { mic: Some("USB Mic".into()), ..Default::default() }.save(&nested).unwrap();
        assert!(!nested.join("settings.json.tmp").exists());
        assert_eq!(Settings::load(&nested).mic.as_deref(), Some("USB Mic"));
    }
}
