#[cfg(feature = "vector-embedder")]
use mcporb_embed::{EmbedderSlot, ModelManager};
use mcporb_runtime_core::{Chunk, Document, OrbManifest, SearchRuntime};
use std::sync::Arc;
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
    /// Source of truth for the on-disk model bundle. Read by both startup and
    /// the hot-load post-download path. Present only in the full build flavor.
    #[cfg(feature = "vector-embedder")]
    #[allow(dead_code)]
    pub model_manager: Arc<ModelManager>,
    /// Hot-swappable embedder. Starts empty; populated on cache-hit at startup
    /// or once the background download completes. See spec §5.5.
    #[cfg(feature = "vector-embedder")]
    pub embedder_slot: Arc<EmbedderSlot>,
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
        #[cfg(feature = "vector-embedder")] model_manager: Arc<ModelManager>,
        #[cfg(feature = "vector-embedder")] embedder_slot: Arc<EmbedderSlot>,
        startup_mode: String,
        orb_binary_path: Option<String>,
        gui_url: Option<String>,
    ) -> SharedState {
        Arc::new(OrbState {
            manifest,
            documents,
            chunks,
            search,
            #[cfg(feature = "vector-embedder")]
            model_manager,
            #[cfg(feature = "vector-embedder")]
            embedder_slot,
            metrics: RwLock::new(Metrics::default()),
            startup_mode,
            orb_binary_path,
            gui_url: RwLock::new(gui_url),
        })
    }
}
