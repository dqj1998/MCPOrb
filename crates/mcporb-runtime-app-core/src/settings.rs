use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSettings {
    #[serde(default = "default_download_dir")]
    pub download_dir: PathBuf,
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default)]
    pub network_binding: NetworkBinding,
    /// User-chosen, user-accessible folder for imported Orb ZIPs (macOS App
    /// Store requirement). `None` = legacy app-data location (Windows/Linux).
    #[serde(default)]
    pub orb_library_dir: Option<PathBuf>,
    /// Base64 security-scoped bookmark for `orb_library_dir` (macOS sandbox only).
    #[serde(default)]
    pub orb_library_bookmark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkBinding {
    Localhost,
    External,
}

impl Default for NetworkBinding {
    fn default() -> Self {
        Self::Localhost
    }
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir(),
            http_port: default_http_port(),
            network_binding: NetworkBinding::Localhost,
            orb_library_dir: None,
            orb_library_bookmark: None,
        }
    }
}

fn default_download_dir() -> PathBuf {
    dirs::download_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Downloads"))
        .join("MCPOrb")
}

fn default_http_port() -> u16 {
    5599
}

pub struct SettingsStore {
    root_dir: PathBuf,
}

impl SettingsStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn default() -> Result<Self> {
        let base = dirs::data_dir()
            .or_else(dirs::config_dir)
            .context("could not resolve user data directory for MCPOrb Runner")?;
        Ok(Self::new(base.join("MCPOrb").join("Runtime")))
    }

    pub fn load(&self) -> Result<RuntimeSettings> {
        let path = self.settings_path();
        if !path.is_file() {
            return Ok(RuntimeSettings::default());
        }
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, settings: &RuntimeSettings) -> Result<()> {
        fs::create_dir_all(&self.root_dir)?;
        let path = self.settings_path();
        let bytes = serde_json::to_string_pretty(settings)?;
        fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
    }

    fn settings_path(&self) -> PathBuf {
        self.root_dir.join(SETTINGS_FILE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_valid() {
        let settings = RuntimeSettings::default();
        assert_eq!(settings.http_port, 5599);
        assert_eq!(settings.network_binding, NetworkBinding::Localhost);
        assert!(settings.orb_library_dir.is_none());
        assert!(settings.orb_library_bookmark.is_none());
    }

    #[test]
    fn roundtrip_settings() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().to_path_buf());
        let mut settings = RuntimeSettings::default();
        settings.http_port = 8080;
        settings.network_binding = NetworkBinding::External;
        settings.orb_library_dir = Some(PathBuf::from("/Users/test/Documents/MCPOrb"));
        settings.orb_library_bookmark = Some("c2VjcmV0".to_string());
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.http_port, 8080);
        assert_eq!(loaded.network_binding, NetworkBinding::External);
        assert_eq!(
            loaded.orb_library_dir.as_deref(),
            Some(std::path::Path::new("/Users/test/Documents/MCPOrb"))
        );
        assert_eq!(loaded.orb_library_bookmark.as_deref(), Some("c2VjcmV0"));
    }

    #[test]
    fn legacy_settings_without_library_fields_still_load() {
        // Old settings.json files (pre-library-folder) must deserialize with
        // the new fields defaulting to None so Windows/macOS upgrades work.
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().to_path_buf());
        std::fs::write(
            store.settings_path(),
            r#"{"download_dir":"/tmp/dl","http_port":5599,"network_binding":"localhost"}"#,
        )
        .unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.http_port, 5599);
        assert!(loaded.orb_library_dir.is_none());
    }
}
