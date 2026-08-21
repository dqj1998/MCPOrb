use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

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
    #[serde(default)]
    pub password_protected: bool,
    pub password_persistence: Option<String>,
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
    // Interior mutability: a mid-session library-folder change repoints
    // new imports via `set_orbs_dir` without rebuilding the store.
    orbs_dir_override: Arc<Mutex<Option<PathBuf>>>,
}

impl RegistryStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            orbs_dir_override: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_orbs_dir(root_dir: PathBuf, orbs_dir: PathBuf) -> Self {
        Self {
            root_dir,
            orbs_dir_override: Arc::new(Mutex::new(Some(orbs_dir))),
        }
    }

    pub fn default() -> Result<Self> {
        let home = dirs::home_dir().context("could not resolve home directory")?;
        let target = home.join(".mcporb");

        // Migration: if registry exists in the old sandbox-container or
        // ~/Library/Application Support location, move it to ~/.mcporb/.
        if !target.join(REGISTRY_FILE).is_file() {
            if let Some(old) = old_registry_path() {
                if old.is_file() {
                    let _ = std::fs::create_dir_all(&target);
                    let _ = std::fs::copy(&old, target.join(REGISTRY_FILE));
                }
            }
        }

        Ok(Self::new(target))
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Where imported Orb ZIPs are stored. Defaults to `<root>/Orbs` — the
    /// layout used on Windows/Linux and on macOS when the user never picked a
    /// custom Orb library folder. `with_orbs_dir` overrides this (macOS
    /// user-chosen library), so ZIPs move to `<library>/Orbs` while
    /// `registry.json` stays at `root_dir`. Consumers must resolve ZIPs via
    /// `InstalledOrb.zip_path` (absolute), never by joining `root_dir` with
    /// `Orbs`.
    pub fn orbs_dir(&self) -> PathBuf {
        self.orbs_dir_override_lock()
            .clone()
            .unwrap_or_else(|| self.root_dir.join(ORBS_DIR))
    }

    /// Repoint where newly imported Orb ZIPs are stored (used after the user
    /// changes the macOS Orb library folder so same-session imports land in
    /// the new folder without a restart).
    pub fn set_orbs_dir(&self, orbs_dir: PathBuf) {
        *self.orbs_dir_override_lock() = Some(orbs_dir);
    }

    fn orbs_dir_override_lock(&self) -> MutexGuard<'_, Option<PathBuf>> {
        self.orbs_dir_override
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Move every Orb ZIP stored under any of `old_orbs_dirs` to
    /// `new_orbs_dir` and rewrite the matching registry entries. Used when the
    /// user changes the Orb library folder and chooses "migrate": the old
    /// library folder and the default app-data `Orbs` dir (legacy imports) are
    /// both sources. Returns the number of orbs moved. Fails before touching
    /// anything if any source ZIP is missing.
    pub fn migrate_orbs(&self, old_orbs_dirs: &[PathBuf], new_orbs_dir: &Path) -> Result<usize> {
        let mut registry = self.load()?;
        let targets: Vec<PathBuf> = registry
            .orbs
            .iter()
            .filter(|orb| {
                old_orbs_dirs.iter().any(|dir| orb.zip_path.starts_with(dir))
                    && !orb.zip_path.starts_with(new_orbs_dir)
            })
            .map(|orb| orb.zip_path.clone())
            .collect();

        for src in &targets {
            if !src.is_file() {
                anyhow::bail!("Orb ZIP `{}` is missing; aborting migration", src.display());
            }
        }

        fs::create_dir_all(new_orbs_dir)
            .with_context(|| format!("create {}", new_orbs_dir.display()))?;

        let mut completed: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(targets.len());
        for src in &targets {
            let result = (|| -> Result<PathBuf> {
                let dest = new_orbs_dir.join(src.file_name().with_context(|| {
                    format!("Orb ZIP `{}` has no file name", src.display())
                })?);
                if dest.exists() {
                    fs::remove_file(&dest)
                        .with_context(|| format!("remove stale {}", dest.display()))?;
                }
                move_file(src, &dest).with_context(|| {
                    format!("move Orb ZIP `{}` to `{}`", src.display(), dest.display())
                })?;
                Ok(dest)
            })();
            match result {
                Ok(dest) => completed.push((src.clone(), dest)),
                Err(error) => {
                    for (moved_from, moved_to) in completed.iter().rev() {
                        let _ = fs::rename(moved_to, moved_from);
                    }
                    return Err(error.context(format!(
                        "Orb ZIP migration failed after moving {} file(s)",
                        completed.len()
                    )));
                }
            }
        }

        for (src, dest) in &completed {
            for orb in &mut registry.orbs {
                if orb.zip_path == *src {
                    orb.zip_path = dest.clone();
                }
            }
        }
        self.save(&registry)?;

        for dir in old_orbs_dirs {
            let _ = fs::remove_dir(dir);
        }
        Ok(targets.len())
    }

    /// Delete every Orb ZIP stored under any of `old_orbs_dirs` and drop the
    /// matching registry entries. Used when the user changes the Orb library
    /// folder and chooses "delete". Returns the number of orbs deleted.
    pub fn delete_orbs(&self, old_orbs_dirs: &[PathBuf]) -> Result<usize> {
        let mut registry = self.load()?;
        let targets: Vec<InstalledOrb> = registry
            .orbs
            .iter()
            .filter(|orb| old_orbs_dirs.iter().any(|dir| orb.zip_path.starts_with(dir)))
            .cloned()
            .collect();

        for orb in &targets {
            if orb.zip_path.is_file() {
                fs::remove_file(&orb.zip_path)
                    .with_context(|| format!("delete Orb ZIP `{}`", orb.zip_path.display()))?;
            }
        }
        registry
            .orbs
            .retain(|orb| !old_orbs_dirs.iter().any(|dir| orb.zip_path.starts_with(dir)));
        self.save(&registry)?;

        for dir in old_orbs_dirs {
            let _ = fs::remove_dir(dir);
        }
        Ok(targets.len())
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
            password_protected: report.password_protected,
            password_persistence: report.password_persistence.clone(),
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

fn old_registry_path() -> Option<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        let sandboxed = home
            .join("Library/Containers/com.mcporb.runner/Data/Library/Application Support/MCPOrb/Runtime");
        if sandboxed.join(REGISTRY_FILE).is_file() {
            return Some(sandboxed.join(REGISTRY_FILE));
        }
    }
    dirs::data_dir()
        .map(|d| d.join("MCPOrb").join("Runtime").join(REGISTRY_FILE))
        .filter(|p| p.is_file())
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

fn move_file(src: &Path, dest: &Path) -> Result<()> {
    if let Err(error) = fs::rename(src, dest) {
        fs::copy(src, dest).with_context(|| {
            format!(
                "copy `{}` to `{}` after rename failed: {error}",
                src.display(),
                dest.display()
            )
        })?;
        fs::remove_file(src).with_context(|| format!("remove `{}` after copy", src.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcporb_runtime_core::format::{Capability, RetrievalPlanKind};

    #[test]
    fn slugifies_names() {
        assert_eq!(slugify("MCP Orb Demo"), "mcp-orb-demo");
        assert_eq!(slugify(" Demo  42 "), "demo-42");
    }

    fn test_manifest() -> OrbManifest {
        OrbManifest {
            name: "test-orb".to_string(),
            display_name: Some("Test Orb".to_string()),
            version: "0.1.0".to_string(),
            description: "registry test".to_string(),
            orb_format_version: "1".to_string(),
            runtime_min_version: None,
            builder_version: None,
            mcp_protocol_version: "2024-11-05".to_string(),
            build_time: "2026-07-03T00:00:00Z".to_string(),
            created_at: None,
            source_documents: vec!["doc.md".to_string()],
            chunk_count: 1,
            index_format_version: "0.2".to_string(),
            binary_size_target_mb: 20,
            assets_sha256: None,
            encrypted: false,
            selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
            enabled_capabilities: vec![Capability::Bm25],
            embedding_dim: None,
            embedding_model: None,
            embedding_model_tar_sha256: None,
            trigram_min_df: None,
            planning_rationale: vec![],
        }
    }

    fn test_orb(id: &str, zip_path: PathBuf) -> InstalledOrb {
        InstalledOrb {
            id: id.to_string(),
            slug: id.to_string(),
            display_name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            manifest: test_manifest(),
            zip_path,
            zip_sha256: id.to_string(),
            assets_sha256: String::new(),
            install_source: InstallSource::LocalImport,
            store_artifact_id: None,
            encrypted_assets: false,
            password_protected: false,
            password_persistence: None,
            last_used_at: None,
        }
    }

    #[test]
    fn set_orbs_dir_repoints_imports() {
        let dir = tempfile::tempdir().unwrap();
        let store = RegistryStore::new(dir.path().to_path_buf());
        assert_eq!(store.orbs_dir(), dir.path().join("Orbs"));

        let other = tempfile::tempdir().unwrap();
        store.set_orbs_dir(other.path().join("Orbs"));
        assert_eq!(store.orbs_dir(), other.path().join("Orbs"));
    }

    fn registry_with_library_orbs(root: &Path, old_orbs_dir: &Path) -> RegistryStore {
        let store = RegistryStore::new(root.to_path_buf());
        fs::create_dir_all(old_orbs_dir).unwrap();
        let zip_a = old_orbs_dir.join("aaaa.orb.zip");
        let zip_b = old_orbs_dir.join("bbbb.orb.zip");
        fs::write(&zip_a, b"zip-a").unwrap();
        fs::write(&zip_b, b"zip-b").unwrap();
        // An orb outside the old library (e.g. imported before the library
        // folder was set, stored in the app-data Orbs dir) must be untouched.
        let zip_outside = root.join("Orbs").join("cccc.orb.zip");
        fs::create_dir_all(root.join("Orbs")).unwrap();
        fs::write(&zip_outside, b"zip-c").unwrap();

        let mut registry = OrbRegistry::default();
        registry.orbs = vec![
            test_orb("a", zip_a),
            test_orb("b", zip_b),
            test_orb("c", zip_outside),
        ];
        store.save(&registry).unwrap();
        store
    }

    #[test]
    fn migrate_orbs_moves_files_and_updates_paths() {
        let root = tempfile::tempdir().unwrap();
        let old_orbs_dir = root.path().join("old").join("Orbs");
        let new_orbs_dir = root.path().join("new").join("Orbs");
        let store = registry_with_library_orbs(root.path(), &old_orbs_dir);

        let moved = store.migrate_orbs(&[old_orbs_dir.clone()], &new_orbs_dir).unwrap();
        assert_eq!(moved, 2);

        assert!(!old_orbs_dir.join("aaaa.orb.zip").exists());
        assert!(new_orbs_dir.join("aaaa.orb.zip").exists());
        assert!(new_orbs_dir.join("bbbb.orb.zip").exists());

        let registry = store.load().unwrap();
        assert_eq!(registry.orbs.len(), 3);
        assert_eq!(
            registry.orbs.iter().find(|o| o.id == "a").unwrap().zip_path,
            new_orbs_dir.join("aaaa.orb.zip")
        );
        assert_eq!(
            registry.orbs.iter().find(|o| o.id == "b").unwrap().zip_path,
            new_orbs_dir.join("bbbb.orb.zip")
        );
        assert_eq!(
            registry.orbs.iter().find(|o| o.id == "c").unwrap().zip_path,
            root.path().join("Orbs").join("cccc.orb.zip")
        );
    }

    #[test]
    fn migrate_orbs_fails_before_moving_when_source_missing() {
        let root = tempfile::tempdir().unwrap();
        let old_orbs_dir = root.path().join("old").join("Orbs");
        let new_orbs_dir = root.path().join("new").join("Orbs");
        let store = registry_with_library_orbs(root.path(), &old_orbs_dir);

        fs::remove_file(old_orbs_dir.join("bbbb.orb.zip")).unwrap();

        let result = store.migrate_orbs(&[old_orbs_dir.clone()], &new_orbs_dir);
        assert!(result.is_err());
        assert!(!new_orbs_dir.exists());
        let registry = store.load().unwrap();
        assert_eq!(registry.orbs.len(), 3);
        assert_eq!(
            registry.orbs.iter().find(|o| o.id == "a").unwrap().zip_path,
            old_orbs_dir.join("aaaa.orb.zip")
        );
        assert!(old_orbs_dir.join("aaaa.orb.zip").exists());
    }

    #[test]
    fn migrate_orbs_moves_from_both_old_library_and_appdata() {
        let root = tempfile::tempdir().unwrap();
        let old_orbs_dir = root.path().join("old").join("Orbs");
        let new_orbs_dir = root.path().join("new").join("Orbs");
        let store = registry_with_library_orbs(root.path(), &old_orbs_dir);

        // Both the old library folder and the default app-data Orbs dir
        // (legacy imports) are migration sources; all three orbs move.
        let appdata_orbs = root.path().join("Orbs");
        let moved = store
            .migrate_orbs(&[old_orbs_dir.clone(), appdata_orbs.clone()], &new_orbs_dir)
            .unwrap();
        assert_eq!(moved, 3);

        let registry = store.load().unwrap();
        assert_eq!(registry.orbs.len(), 3);
        for orb in &registry.orbs {
            assert!(orb.zip_path.starts_with(&new_orbs_dir));
        }
        assert!(new_orbs_dir.join("cccc.orb.zip").exists());
        assert!(!appdata_orbs.join("cccc.orb.zip").exists());
    }

    #[test]
    fn delete_orbs_removes_files_and_entries() {
        let root = tempfile::tempdir().unwrap();
        let old_orbs_dir = root.path().join("old").join("Orbs");
        let store = registry_with_library_orbs(root.path(), &old_orbs_dir);

        let deleted = store.delete_orbs(&[old_orbs_dir.clone()]).unwrap();
        assert_eq!(deleted, 2);

        assert!(!old_orbs_dir.join("aaaa.orb.zip").exists());
        assert!(!old_orbs_dir.join("bbbb.orb.zip").exists());
        let registry = store.load().unwrap();
        assert_eq!(registry.orbs.len(), 1);
        assert_eq!(registry.orbs[0].id, "c");
        assert_eq!(
            registry.orbs[0].zip_path,
            root.path().join("Orbs").join("cccc.orb.zip")
        );
    }

    #[test]
    fn migrate_orbs_rolls_back_moved_files_when_later_move_fails() {
        let root = tempfile::tempdir().unwrap();
        let old_orbs_dir = root.path().join("old").join("Orbs");
        let new_orbs_dir = root.path().join("new").join("Orbs");
        let store = registry_with_library_orbs(root.path(), &old_orbs_dir);

        // Sabotage the second move: a directory squatting on the destination
        // path makes `remove_file` fail after the first file already moved.
        fs::create_dir_all(&new_orbs_dir).unwrap();
        fs::create_dir(new_orbs_dir.join("bbbb.orb.zip")).unwrap();

        let result = store.migrate_orbs(&[old_orbs_dir.clone()], &new_orbs_dir);
        assert!(result.is_err(), "migration must fail on the blocked file");

        // The first file must be rolled back to its original location and the
        // registry file must still point at the old directory.
        assert!(
            old_orbs_dir.join("aaaa.orb.zip").exists(),
            "already-moved file must be rolled back to the old library"
        );
        assert!(
            !new_orbs_dir.join("aaaa.orb.zip").exists(),
            "no file may remain in the new directory after rollback"
        );
        let registry = store.load().unwrap();
        assert_eq!(registry.orbs.len(), 3);
        for orb in &registry.orbs {
            assert!(
                orb.zip_path.starts_with(&old_orbs_dir) || orb.zip_path.starts_with(root.path().join("Orbs")),
                "registry must be untouched after rollback; got {}",
                orb.zip_path.display()
            );
        }
    }

    #[test]
    fn delete_orbs_tolerates_missing_zip_and_cleans_empty_dirs() {
        let root = tempfile::tempdir().unwrap();
        let old_orbs_dir = root.path().join("old").join("Orbs");
        let store = registry_with_library_orbs(root.path(), &old_orbs_dir);

        // A zip already gone from disk (manual cleanup, failed install, …)
        // must not abort the deletion of the remaining orbs.
        fs::remove_file(old_orbs_dir.join("aaaa.orb.zip")).unwrap();

        let deleted = store.delete_orbs(&[old_orbs_dir.clone()]).unwrap();
        assert_eq!(deleted, 2, "both orbs are deleted even if one zip is missing");

        assert!(!old_orbs_dir.join("bbbb.orb.zip").exists());
        assert!(
            !old_orbs_dir.exists(),
            "empty old library directory must be removed after deletion"
        );
        let registry = store.load().unwrap();
        assert_eq!(registry.orbs.len(), 1);
        assert_eq!(registry.orbs[0].id, "c");
    }
}
