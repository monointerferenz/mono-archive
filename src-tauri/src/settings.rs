//! Application settings persisted in the app config directory.
//!
//! Empty fields mean "not configured"; the backend falls back to detected
//! defaults so a fresh checkout works without setup.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Directory holding the project folders.
    pub archive_path: String,
    /// Engine project root (contains `themes/`, `scripts/`).
    pub engine_root: String,
    /// Path to the `monointerferenz-engine` binary.
    pub engine_binary: String,
    /// Project template directory cloned for new archives.
    pub template_path: String,
}

impl Settings {
    /// Effective values with defaults filled in for empty fields.
    pub fn resolved(&self) -> Settings {
        Settings {
            archive_path: self.archive_path.clone(),
            engine_root: if self.engine_root.is_empty() {
                paths::default_engine_root()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            } else {
                self.engine_root.clone()
            },
            engine_binary: if self.engine_binary.is_empty() {
                paths::default_engine_binary()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            } else {
                self.engine_binary.clone()
            },
            template_path: if self.template_path.is_empty() {
                paths::default_template_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            } else {
                self.template_path.clone()
            },
        }
    }
}

fn settings_file(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

pub fn load(config_dir: &Path) -> Settings {
    let path = settings_file(config_dir);
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Settings>(&raw).ok())
        .unwrap_or_default()
        .resolved()
}

pub fn save(config_dir: &Path, settings: &Settings) -> std::io::Result<()> {
    fs::create_dir_all(config_dir)?;
    let resolved = settings.resolved();
    let raw = serde_json::to_string_pretty(&resolved)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(settings_file(config_dir), raw + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_disk() {
        let dir =
            std::env::temp_dir().join(format!("mono-archive-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        let s = Settings {
            archive_path: "/tmp/archive".into(),
            ..Default::default()
        };
        save(&dir, &s).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.archive_path, "/tmp/archive");
        // Defaults filled in on load.
        assert!(!loaded.engine_root.is_empty() || paths::default_engine_root().is_none());

        fs::remove_dir_all(&dir).ok();
    }
}
