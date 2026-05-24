use serde::{Deserialize, Serialize};

/// The retrieval plan selected at build time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalPlanKind {
    Bm25Only,
    Bm25FlatVector,
    Bm25Hnsw,
    Bm25KnowledgeGraph,
    Bm25HnswKnowledgeGraph,
}

impl Default for RetrievalPlanKind {
    fn default() -> Self {
        RetrievalPlanKind::Bm25Only
    }
}

impl std::fmt::Display for RetrievalPlanKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetrievalPlanKind::Bm25Only => write!(f, "bm25-only"),
            RetrievalPlanKind::Bm25FlatVector => write!(f, "bm25-flat-vector"),
            RetrievalPlanKind::Bm25Hnsw => write!(f, "bm25-hnsw"),
            RetrievalPlanKind::Bm25KnowledgeGraph => write!(f, "bm25-kg"),
            RetrievalPlanKind::Bm25HnswKnowledgeGraph => write!(f, "bm25-hnsw-kg"),
        }
    }
}

/// Individual retrieval capabilities enabled in this Orb.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Bm25,
    TfIdf,
    FlatVector,
    Hnsw,
    Trigram,
    KnowledgeGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub orb_format_version: String,
    pub mcp_protocol_version: String,
    pub build_time: String,
    pub source_documents: Vec<String>,
    pub chunk_count: usize,
    pub index_format_version: String,
    pub binary_size_target_mb: u32,
    /// The retrieval plan selected at build time (default: bm25_only).
    #[serde(default)]
    pub selected_retrieval_plan: RetrievalPlanKind,
    /// The capabilities enabled in this Orb.
    #[serde(default)]
    pub enabled_capabilities: Vec<Capability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_dim: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    /// SHA256 of the embedder bundle tar.zst the Builder used. Runtime hard-rejects
    /// dense queries when this is `Some(_)` and disagrees with its compiled-in
    /// `mcporb_embed::MODEL_TAR_SHA256`. Older Orbs (`None`) fall back to soft
    /// constraint (dim check only) per spec §4.5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model_tar_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigram_min_df: Option<usize>,
    /// Rationale for the selected plan. Stored as opaque `Value` because the
    /// Builder emits structured `{key, params}` objects while older Orbs may
    /// emit plain strings; Runtime only forwards it to the GUI verbatim.
    #[serde(default)]
    pub planning_rationale: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: u32,
    pub title: String,
    pub source_path: String,
    pub page_count: Option<usize>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: u32,
    pub document_id: u32,
    pub title: String,
    pub page_start: Option<u32>,
    pub page_end: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: u32,
    pub document_id: u32,
    pub section_id: Option<u32>,
    pub page: Option<u32>,
    pub text: String,
    pub token_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bm25Index {
    pub doc_count: usize,
    pub avg_doc_len: f32,
    pub vocab: std::collections::HashMap<String, u32>,
    pub postings: Vec<Vec<(u32, f32)>>,
    pub doc_lengths: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TfIdfIndex {
    pub doc_count: usize,
    pub vocab: std::collections::HashMap<String, u32>,
    pub idf: Vec<f32>,
    pub doc_vectors: Vec<Vec<(u32, f32)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlatVectorIndex {
    pub chunk_count: usize,
    pub dim: usize,
    pub vectors: Vec<f32>,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HnswIndex {
    pub chunk_count: usize,
    pub dim: usize,
    pub m: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub model_id: String,
    pub graph_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrigramIndex {
    pub doc_count: usize,
    pub vocab: std::collections::HashMap<String, u32>,
    pub postings: Vec<Vec<u32>>,
    pub trigram_counts: Vec<u32>,
}
