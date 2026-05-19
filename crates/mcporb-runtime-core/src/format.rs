use serde::{Deserialize, Serialize};

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
