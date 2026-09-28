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
    /// "auto" or an ISO 639-1 code. Parakeet auto-detects; this is a hint and a label.
    pub language: String,
    /// Free the model from RAM after this many idle minutes (0 = never).
    pub idle_unload_min: u32,
    pub mascot_enabled: bool,
    /// "glass" | "jolly" | "dot" | "aura"
    pub mascot_skin: String,
    /// "s" | "m" | "l"
    pub mascot_size: String,
    /// Input device name; None = system default.
    pub mic: Option<String>,
    pub launch_at_login: bool,
    /// Warn in the app when total storage (model + history) exceeds this.
    pub storage_warn_mb: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey().into(),
            language: "auto".into(),
            idle_unload_min: 5,
            mascot_enabled: true,
            mascot_skin: "glass".into(),
            mascot_size: "m".into(),
            mic: None,
            launch_at_login: false,
            storage_warn_mb: 2000,
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

pub const SKINS: [&str; 4] = ["glass", "jolly", "dot", "aura"];
pub const SIZES: [&str; 3] = ["s", "m", "l"];

impl Settings {
    /// Fixes out-of-range values coming from the UI or an old settings file.
    pub fn sanitized(mut self) -> Self {
        if !SKINS.contains(&self.mascot_skin.as_str()) {
            self.mascot_skin = "glass".into();
        }
        if !SIZES.contains(&self.mascot_size.as_str()) {
            self.mascot_size = "m".into();
        }
        self
    }

    pub fn load(dir: &Path) -> Settings {
        std::fs::read_to_string(dir.join(FILE))
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(FILE), serde_json::to_string_pretty(self)?)?;
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
        assert_eq!(s.idle_unload_min, 5);
    }
}
