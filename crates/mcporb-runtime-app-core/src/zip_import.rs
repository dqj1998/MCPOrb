use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use mcporb_runtime_core::OrbManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ORB_ZIP_FORMAT_VERSION: &str = "1";
pub const RUNTIME_APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_ZIP_FILES: usize = 200;
pub const MAX_FILE_SIZE: u64 = 512 * 1024 * 1024;
pub const MAX_TOTAL_SIZE: u64 = 3 * 1024 * 1024 * 1024;

const REQUIRED_PLAINTEXT_FILES: &[&str] = &[
    "orb_manifest.json",
    "documents.postcard",
    "chunks.postcard",
    "bm25_index.postcard",
];
const OPTIONAL_FILES: &[&str] = &[
    "tfidf_index.postcard",
    "trigram_index.postcard",
    "vector_store.postcard",
    "hnsw_index.postcard",
    "orb_security.json",
    "orb_assets.enc",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub copy_into_registry: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            copy_into_registry: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipValidationReport {
    pub manifest: OrbManifest,
    pub zip_sha256: String,
    pub assets_sha256: String,
    pub file_count: usize,
    pub total_uncompressed_size: u64,
    pub encrypted_assets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub report: ZipValidationReport,
    pub stored_zip_path: PathBuf,
}

pub fn validate_zip_path(path: &Path) -> Result<ZipValidationReport> {
    let bytes = fs::read(path).with_context(|| format!("read Orb ZIP {}", path.display()))?;
    validate_zip_bytes(&bytes)
}

pub fn validate_zip_bytes(bytes: &[u8]) -> Result<ZipValidationReport> {
    let zip_sha256 = sha256_hex(bytes);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    validate_archive(&mut archive, zip_sha256)
}

fn validate_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    zip_sha256: String,
) -> Result<ZipValidationReport> {
    if archive.len() > MAX_ZIP_FILES {
        bail!(
            "Orb ZIP contains {} files; maximum is {}",
            archive.len(),
            MAX_ZIP_FILES
        );
    }

    let mut names = Vec::with_capacity(archive.len());
    let mut total_size = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = file.name().to_string();
        validate_entry_name(&name)?;
        if !is_allowed_file(&name) {
            bail!("Orb ZIP contains unsupported file `{name}`");
        }
        if file.is_dir() {
            bail!("Orb ZIP contains directory entry `{name}`");
        }
        if file.size() > MAX_FILE_SIZE {
            bail!("Orb ZIP file `{name}` exceeds the 512 MB per-file limit");
        }
        total_size = total_size
            .checked_add(file.size())
            .context("Orb ZIP uncompressed size overflow")?;
        if total_size > MAX_TOTAL_SIZE {
            bail!("Orb ZIP exceeds the 3 GB total uncompressed limit");
        }
        names.push(name);
    }

    let encrypted_assets = names.iter().any(|name| name == "orb_assets.enc");
    if encrypted_assets {
        ensure_present(&names, "orb_security.json")?;
        ensure_present(&names, "orb_assets.enc")?;
        if !names.iter().any(|name| name == "orb_manifest.json") {
            bail!(
                "Encrypted Orb ZIP does not expose orb_manifest.json; export Orb ZIP v1 with a public manifest before importing into Runtime App"
            );
        }
    } else {
        for required in REQUIRED_PLAINTEXT_FILES {
            ensure_present(&names, required)?;
        }
    }

    let manifest_json = read_file(archive, "orb_manifest.json")?;
    let manifest: OrbManifest = serde_json::from_slice(&manifest_json)
        .context("parse orb_manifest.json from Orb ZIP")?;
    validate_manifest_versions(&manifest)?;

    let assets_sha256 = compute_assets_sha256(archive, &names)?;
    if let Some(expected) = manifest.assets_sha256.as_deref() {
        if expected != assets_sha256 {
            bail!(
                "Orb ZIP assets_sha256 mismatch: manifest has {expected}, calculated {assets_sha256}"
            );
        }
    }

    Ok(ZipValidationReport {
        manifest,
        zip_sha256,
        assets_sha256,
        file_count: names.len(),
        total_uncompressed_size: total_size,
        encrypted_assets,
    })
}

fn validate_manifest_versions(manifest: &OrbManifest) -> Result<()> {
    if manifest.orb_format_version.as_str() > ORB_ZIP_FORMAT_VERSION {
        bail!(
            "Orb format {} is newer than this Runtime App supports ({})",
            manifest.orb_format_version,
            ORB_ZIP_FORMAT_VERSION
        );
    }
    if let Some(min_version) = manifest.runtime_min_version.as_deref() {
        if compare_semverish(min_version, RUNTIME_APP_VERSION).is_gt() {
            bail!(
                "Orb requires MCPOrb Runtime {min_version} or newer; this app is {RUNTIME_APP_VERSION}"
            );
        }
    }
    Ok(())
}

fn compare_semverish(left: &str, right: &str) -> std::cmp::Ordering {
    let parse = |value: &str| {
        value
            .split(|ch: char| !ch.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .take(3)
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let mut left_parts = parse(left);
    let mut right_parts = parse(right);
    left_parts.resize(3, 0);
    right_parts.resize(3, 0);
    left_parts.cmp(&right_parts)
}

fn compute_assets_sha256<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    names: &[String],
) -> Result<String> {
    let mut asset_names = names
        .iter()
        .filter(|name| name.as_str() != "orb_manifest.json")
        .cloned()
        .collect::<Vec<_>>();
    asset_names.sort();

    let mut hasher = Sha256::new();
    for name in asset_names {
        let bytes = read_file(archive, &name)?;
        hasher.update(bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_file<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
    let mut file = archive.by_name(name)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn ensure_present(names: &[String], required: &str) -> Result<()> {
    if names.iter().any(|name| name == required) {
        Ok(())
    } else {
        bail!("Orb ZIP is missing required file `{required}`")
    }
}

fn validate_entry_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains('\\') || name.starts_with('/') {
        bail!("Orb ZIP contains invalid path `{name}`");
    }
    let path = Path::new(name);
    if path.components().count() != 1 {
        bail!("Orb ZIP entry `{name}` must be a top-level file");
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => bail!("Orb ZIP contains unsafe path `{name}`"),
        }
    }
    Ok(())
}

fn is_allowed_file(name: &str) -> bool {
    REQUIRED_PLAINTEXT_FILES.iter().any(|allowed| name == *allowed)
        || OPTIONAL_FILES.iter().any(|allowed| name == *allowed)
}

pub fn copy_zip_into_registry(source: &Path, registry_dir: &Path, zip_sha256: &str) -> Result<PathBuf> {
    fs::create_dir_all(registry_dir)?;
    let target = registry_dir.join(format!("{zip_sha256}.orb.zip"));
    fs::copy(source, &target)
        .with_context(|| format!("copy Orb ZIP into {}", target.display()))?;
    Ok(target)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use mcporb_runtime_core::format::{Capability, RetrievalPlanKind};
    use mcporb_runtime_core::{Bm25Index, Chunk, Document};

    fn test_manifest(assets_sha256: Option<String>) -> OrbManifest {
        OrbManifest {
            name: "test-orb".to_string(),
            display_name: Some("Test Orb".to_string()),
            version: "0.1.0".to_string(),
            description: "runtime app import test".to_string(),
            orb_format_version: "1".to_string(),
            runtime_min_version: None,
            builder_version: Some("1.2.1".to_string()),
            mcp_protocol_version: "2024-11-05".to_string(),
            build_time: "2026-07-03T00:00:00Z".to_string(),
            created_at: Some("2026-07-03T00:00:00Z".to_string()),
            source_documents: vec!["doc.md".to_string()],
            chunk_count: 1,
            index_format_version: "0.2".to_string(),
            binary_size_target_mb: 20,
            assets_sha256,
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

    fn build_zip(include_hash: bool) -> Vec<u8> {
        let documents = vec![Document {
            id: 0,
            title: "Doc".to_string(),
            source_path: "doc.md".to_string(),
            page_count: None,
            sections: vec![],
        }];
        let chunks = vec![Chunk {
            id: 0,
            document_id: 0,
            section_id: None,
            page: None,
            text: "hello runtime app".to_string(),
            token_count: 3,
        }];
        let index = Bm25Index::default();
        let docs = postcard::to_allocvec(&documents).unwrap();
        let chunk_bytes = postcard::to_allocvec(&chunks).unwrap();
        let bm25 = postcard::to_allocvec(&index).unwrap();
        let expected_assets_sha = {
            let mut hasher = Sha256::new();
            for bytes in [&bm25, &chunk_bytes, &docs] {
                hasher.update(bytes);
            }
            format!("{:x}", hasher.finalize())
        };
        let manifest = test_manifest(include_hash.then_some(expected_assets_sha));

        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("orb_manifest.json", opts).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        zip.start_file("documents.postcard", opts).unwrap();
        zip.write_all(&docs).unwrap();
        zip.start_file("chunks.postcard", opts).unwrap();
        zip.write_all(&chunk_bytes).unwrap();
        zip.start_file("bm25_index.postcard", opts).unwrap();
        zip.write_all(&bm25).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn validates_plaintext_zip_v1() {
        let zip = build_zip(true);
        let report = validate_zip_bytes(&zip).unwrap();
        assert_eq!(report.manifest.name, "test-orb");
        assert_eq!(report.file_count, 4);
        assert!(!report.encrypted_assets);
    }

    #[test]
    fn rejects_path_traversal() {
        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        zip.start_file("../orb_manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"{}").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(validate_zip_bytes(&bytes).is_err());
    }
}
