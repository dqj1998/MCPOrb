use std::sync::Arc;
use mcporb_runtime_core::{Chunk, Document, OrbManifest, SearchRuntime};
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
    pub search: SearchRuntime,
    pub metrics: RwLock<Metrics>,
    pub startup_mode: String,
    pub orb_binary_path: Option<String>,
    pub gui_url: RwLock<Option<String>>,
}

pub type SharedState = Arc<OrbState>;

impl OrbState {
    pub fn new(
        manifest: OrbManifest,
        documents: Vec<Document>,
        chunks: Vec<Chunk>,
        search: SearchRuntime,
        startup_mode: String,
        orb_binary_path: Option<String>,
        gui_url: Option<String>,
    ) -> SharedState {
        Arc::new(OrbState {
            manifest,
            documents,
            chunks,
            search,
            metrics: RwLock::new(Metrics::default()),
            startup_mode,
            orb_binary_path,
            gui_url: RwLock::new(gui_url),
        })
    }
}
