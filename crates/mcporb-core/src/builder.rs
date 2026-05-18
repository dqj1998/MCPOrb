use std::path::{Path, PathBuf};
use crate::error::OrbError;
use crate::format::{Chunk, Document, OrbManifest};
use crate::importer::{markdown::MarkdownImporter, pdf::PdfImporter, DocumentImporter};
use crate::chunker::{chunk_raw, ChunkerConfig};

pub struct OrbBuildConfig {
    pub name: String,
    pub description: String,
    pub output_dir: PathBuf,
    pub chunker: ChunkerConfig,
}

pub struct OrbBuildResult {
    pub manifest: OrbManifest,
    pub documents: Vec<Document>,
    pub chunks: Vec<Chunk>,
    pub output_dir: PathBuf,
}

pub fn build_orb(source: &Path, config: OrbBuildConfig) -> Result<OrbBuildResult, OrbError> {
    // Select importer based on extension
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let import_result = match ext.as_str() {
        "md" | "markdown" => MarkdownImporter.import(source)?,
        "pdf" => PdfImporter.import(source)?,
        other => {
            return Err(OrbError::DocumentProcessing(format!(
                "Unsupported file extension: .{other}"
            )))
        }
    };

    let mut document = import_result.document;
    document.id = 0;
    // Fix section document_ids
    for section in &mut document.sections {
        section.document_id = 0;
    }

    let chunks = chunk_raw(import_result.raw_chunks, 0, &config.chunker);

    let now = chrono::Utc::now().to_rfc3339();
    let manifest = OrbManifest {
        name: config.name.clone(),
        version: "0.1.0".to_string(),
        description: config.description,
        orb_format_version: "0.1".to_string(),
        mcp_protocol_version: "2024-11-05".to_string(),
        build_time: now,
        source_documents: vec![source.to_string_lossy().to_string()],
        chunk_count: chunks.len(),
        index_format_version: "0.1".to_string(),
        binary_size_target_mb: 15,
    };

    // Serialize assets
    std::fs::create_dir_all(&config.output_dir).map_err(OrbError::Io)?;

    // manifest.json
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| OrbError::Serialization(e.to_string()))?;
    std::fs::write(config.output_dir.join("orb_manifest.json"), manifest_json)
        .map_err(OrbError::Io)?;

    // documents.postcard
    let docs_bytes = postcard::to_allocvec(&vec![document.clone()])
        .map_err(|e| OrbError::Serialization(e.to_string()))?;
    std::fs::write(config.output_dir.join("documents.postcard"), docs_bytes)
        .map_err(OrbError::Io)?;

    // chunks.postcard
    let chunks_bytes =
        postcard::to_allocvec(&chunks).map_err(|e| OrbError::Serialization(e.to_string()))?;
    std::fs::write(config.output_dir.join("chunks.postcard"), chunks_bytes)
        .map_err(OrbError::Io)?;

    // bm25_index.postcard — real BM25 index built from chunks
    let index = crate::bm25::build_index(&chunks);
    let index_bytes =
        postcard::to_allocvec(&index).map_err(|e| OrbError::Serialization(e.to_string()))?;
    std::fs::write(config.output_dir.join("bm25_index.postcard"), index_bytes)
        .map_err(OrbError::Io)?;

    tracing::info!(
        name = %config.name,
        chunks = chunks.len(),
        output = %config.output_dir.display(),
        "Orb build complete"
    );

    Ok(OrbBuildResult {
        manifest,
        documents: vec![document],
        chunks,
        output_dir: config.output_dir,
    })
}
