use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use anyhow::{Context, Result};
use mcporb_runtime_core::format::Capability;
use mcporb_runtime_core::{
    Bm25Index, Chunk, DenseRuntime, Document, FlatVectorIndex, HnswIndex, OrbManifest,
    SearchMethodRequest, SearchRequest, SearchRuntime, TfIdfIndex, TrigramIndex,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub chunk_id: u32,
    pub score: f32,
    pub method: String,
    pub text: String,
    pub document_title: String,
    pub source_path: String,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub orb_name: String,
    pub active_plan: String,
    pub hits: Vec<SearchHit>,
}

struct LoadedKnowledge {
    manifest: OrbManifest,
    documents: Vec<Document>,
    chunks: Vec<Chunk>,
    search: SearchRuntime,
}

pub fn search_zip(
    zip_path: &Path,
    query: &str,
    method: Option<&str>,
    top_k: Option<usize>,
) -> Result<SearchResponse> {
    let query = query.trim();
    if query.is_empty() {
        anyhow::bail!("query cannot be empty");
    }
    let knowledge = load_plaintext_zip(zip_path)?;
    let response = knowledge.search.search(&SearchRequest {
        query: query.to_string(),
        top_k: top_k.unwrap_or(8).clamp(1, 50),
        method: method
            .map(SearchMethodRequest::from_str)
            .unwrap_or(SearchMethodRequest::Auto),
        query_vector: None,
        explain: false,
    })?;

    let documents = knowledge
        .documents
        .iter()
        .map(|doc| (doc.id, doc))
        .collect::<HashMap<_, _>>();
    let chunks = knowledge
        .chunks
        .iter()
        .map(|chunk| (chunk.id, chunk))
        .collect::<HashMap<_, _>>();

    let hits = response
        .hits
        .into_iter()
        .filter_map(|hit| {
            let chunk = chunks.get(&hit.chunk_id)?;
            let doc = documents.get(&chunk.document_id)?;
            Some(SearchHit {
                chunk_id: hit.chunk_id,
                score: hit.score,
                method: hit.method.to_string(),
                text: chunk.text.clone(),
                document_title: doc.title.clone(),
                source_path: doc.source_path.clone(),
                page: chunk.page,
            })
        })
        .collect();

    Ok(SearchResponse {
        orb_name: knowledge
            .manifest
            .display_name
            .clone()
            .unwrap_or_else(|| knowledge.manifest.name.clone()),
        active_plan: response.active_plan.to_string(),
        hits,
    })
}

fn load_plaintext_zip(zip_path: &Path) -> Result<LoadedKnowledge> {
    let bytes = fs::read(zip_path).with_context(|| format!("read {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.by_name("orb_assets.enc").is_ok() {
        anyhow::bail!("encrypted Orb search is not available until the Orb is unlocked");
    }
    read_knowledge_from_archive(&mut archive)
}

fn read_knowledge_from_archive<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<LoadedKnowledge> {
    let manifest_json = read_bundle_asset(archive, "orb_manifest.json")?;
    let documents_bytes = read_bundle_asset(archive, "documents.postcard")?;
    let chunks_bytes = read_bundle_asset(archive, "chunks.postcard")?;
    let bm25_bytes = read_bundle_asset(archive, "bm25_index.postcard")?;
    let tfidf_bytes = read_optional_bundle_asset(archive, "tfidf_index.postcard")?;
    let trigram_bytes = read_optional_bundle_asset(archive, "trigram_index.postcard")?;
    let vector_bytes = read_optional_bundle_asset(archive, "vector_store.postcard")?;
    let hnsw_bytes = read_optional_bundle_asset(archive, "hnsw_index.postcard")?;

    let manifest: OrbManifest = serde_json::from_slice(&manifest_json)?;
    let documents: Vec<Document> = postcard::from_bytes(&documents_bytes)?;
    let chunks: Vec<Chunk> = postcard::from_bytes(&chunks_bytes)?;
    let bm25: Bm25Index = postcard::from_bytes(&bm25_bytes)?;
    let tfidf = load_optional_index::<TfIdfIndex>(&manifest, Capability::TfIdf, tfidf_bytes.as_deref())?;
    let trigram = load_optional_index::<TrigramIndex>(&manifest, Capability::Trigram, trigram_bytes.as_deref())?;
    let vector = load_optional_index::<FlatVectorIndex>(&manifest, Capability::FlatVector, vector_bytes.as_deref())?;
    let hnsw = load_optional_index::<HnswIndex>(&manifest, Capability::Hnsw, hnsw_bytes.as_deref())?;

    Ok(LoadedKnowledge {
        search: SearchRuntime {
            bm25,
            tfidf,
            trigram,
            dense: DenseRuntime::from_assets(vector, hnsw)?,
            dense_tier: manifest.selected_retrieval_plan.clone(),
        },
        manifest,
        documents,
        chunks,
    })
}

fn read_bundle_asset<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
    let mut file = archive.by_name(name)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_optional_bundle_asset<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn load_optional_index<T>(
    manifest: &OrbManifest,
    capability: Capability,
    bytes: Option<&[u8]>,
) -> Result<Option<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let capability_enabled = manifest
        .enabled_capabilities
        .iter()
        .any(|value| *value == capability);

    match (capability_enabled, bytes) {
        (true, Some(bytes)) => Ok(Some(postcard::from_bytes(bytes)?)),
        (true, None) => anyhow::bail!("missing asset for enabled capability {:?}", capability),
        (false, Some(bytes)) => Ok(Some(postcard::from_bytes(bytes)?)),
        (false, None) => Ok(None),
    }
}
