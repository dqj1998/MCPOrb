use std::sync::Arc;
use mcporb_runtime_core::{Bm25Index, Chunk, Document, OrbManifest};
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub struct Metrics {
    pub mcp_request_count: u64,
    pub search_count: u64,
}

pub struct OrbState {
    pub manifest: OrbManifest,
    pub documents: Vec<Document>,
    pub chunks: Vec<Chunk>,
    pub index: Bm25Index,
    pub metrics: RwLock<Metrics>,
    pub startup_mode: String,
    pub gui_url: RwLock<Option<String>>,
}

pub type SharedState = Arc<OrbState>;

impl OrbState {
    pub fn new(
        manifest: OrbManifest,
        documents: Vec<Document>,
        chunks: Vec<Chunk>,
        index: Bm25Index,
        startup_mode: String,
        gui_url: Option<String>,
    ) -> SharedState {
        Arc::new(OrbState {
            manifest,
            documents,
            chunks,
            index,
            metrics: RwLock::new(Metrics::default()),
            startup_mode,
            gui_url: RwLock::new(gui_url),
        })
    }
}
