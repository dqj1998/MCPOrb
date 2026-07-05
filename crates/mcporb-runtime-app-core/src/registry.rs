use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mcporb_runtime_core::OrbManifest;
use serde::{Deserialize, Serialize};

use crate::zip_import::{copy_zip_into_registry, validate_zip_path, ImportOptions, ImportResult};

const REGISTRY_FILE: &str = "registry.json";
const ORBS_DIR: &str = "Orbs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledOrb {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub manifest: OrbManifest,
    pub zip_path: PathBuf,
    pub zip_sha256: String,
    pub assets_sha256: String,
    pub install_source: InstallSource,
    pub store_artifact_id: Option<String>,
    pub encrypted_assets: bool,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallSource {
    LocalImport,
    StoreDownload,
    LegacyOrbImport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrbRegistry {
    pub orbs: Vec<InstalledOrb>,
}

#[derive(Debug, Clone)]
pub struct RegistryStore {
    root_dir: PathBuf,
}

impl RegistryStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn default() -> Result<Self> {
        let base = dirs::data_dir()
            .or_else(dirs::config_dir)
            .context("could not resolve user data directory for MCPOrb Runtime")?;
        Ok(Self::new(base.join("MCPOrb").join("Runtime")))
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn orbs_dir(&self) -> PathBuf {
        self.root_dir.join(ORBS_DIR)
    }

    pub fn load(&self) -> Result<OrbRegistry> {
        let path = self.registry_path();
        if !path.is_file() {
            return Ok(OrbRegistry::default());
        }
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, registry: &OrbRegistry) -> Result<()> {
        fs::create_dir_all(&self.root_dir)?;
        let path = self.registry_path();
        let bytes = serde_json::to_vec_pretty(registry)?;
        fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
    }

    pub fn list(&self) -> Result<Vec<InstalledOrb>> {
        Ok(self.load()?.orbs)
    }

    pub fn get(&self, id: &str) -> Result<Option<InstalledOrb>> {
        Ok(self.load()?.orbs.into_iter().find(|orb| orb.id == id))
    }

    /// Remove an installed orb by ID: deletes its ZIP file from disk and
    /// removes it from the registry. Returns the removed orb info.
    pub fn remove(&self, id: &str) -> Result<InstalledOrb> {
        let mut registry = self.load()?;
        let pos = registry
            .orbs
            .iter()
            .position(|orb| orb.id == id)
            .ok_or_else(|| anyhow::anyhow!("Orb `{id}` is not installed"))?;
        let orb = registry.orbs.remove(pos);
        if orb.zip_path.exists() {
            fs::remove_file(&orb.zip_path)
                .with_context(|| format!("delete orb ZIP `{}`", orb.zip_path.display()))?;
        }
        self.save(&registry)?;
        Ok(orb)
    }

    pub fn import_zip(&self, source_zip: &Path, options: ImportOptions) -> Result<ImportResult> {
        let report = validate_zip_path(source_zip)?;
        let stored_zip_path = if options.copy_into_registry {
            copy_zip_into_registry(source_zip, &self.orbs_dir(), &report.zip_sha256)?
        } else {
            source_zip.to_path_buf()
        };

        let mut registry = self.load()?;
        let installed = InstalledOrb {
            id: report.zip_sha256[..16].to_string(),
            slug: slugify(&report.manifest.name),
            display_name: report
                .manifest
                .display_name
                .clone()
                .unwrap_or_else(|| report.manifest.name.clone()),
            version: report.manifest.version.clone(),
            description: report.manifest.description.clone(),
            manifest: report.manifest.clone(),
            zip_path: stored_zip_path.clone(),
            zip_sha256: report.zip_sha256.clone(),
            assets_sha256: report.assets_sha256.clone(),
            install_source: InstallSource::LocalImport,
            store_artifact_id: None,
            encrypted_assets: report.encrypted_assets,
            last_used_at: None,
        };
        upsert_orb(&mut registry, installed);
        self.save(&registry)?;

        Ok(ImportResult {
            report,
            stored_zip_path,
        })
    }

    fn registry_path(&self) -> PathBuf {
        self.root_dir.join(REGISTRY_FILE)
    }
}

fn upsert_orb(registry: &mut OrbRegistry, installed: InstalledOrb) {
    if let Some(existing) = registry
        .orbs
        .iter_mut()
        .find(|orb| orb.zip_sha256 == installed.zip_sha256)
    {
        *existing = installed;
        return;
    }
    registry.orbs.push(installed);
    registry
        .orbs
        .sort_by(|left, right| left.display_name.cmp(&right.display_name));
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_names() {
        assert_eq!(slugify("MCP Orb Demo"), "mcp-orb-demo");
        assert_eq!(slugify(" Demo  42 "), "demo-42");
    }
}
