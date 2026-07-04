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
    #[serde(default = "default_true")]
    pub auto_start: bool,
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
            auto_start: true,
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

fn default_true() -> bool {
    true
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
            .context("could not resolve user data directory for MCPOrb Runtime")?;
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
        assert!(settings.auto_start);
    }

    #[test]
    fn roundtrip_settings() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().to_path_buf());
        let mut settings = RuntimeSettings::default();
        settings.http_port = 8080;
        settings.network_binding = NetworkBinding::External;
        settings.auto_start = false;
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.http_port, 8080);
        assert_eq!(loaded.network_binding, NetworkBinding::External);
        assert!(!loaded.auto_start);
    }
}
