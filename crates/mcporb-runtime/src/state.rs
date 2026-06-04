#[cfg(feature = "vector-embedder")]
use mcporb_embed::{EmbedderSlot, ModelManager};
use mcporb_runtime_core::{Chunk, Document, OrbManifest, SearchRuntime};
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

use crate::security::{SecurityConfig, SecurityState};

#[derive(Debug, Default)]
pub struct Metrics {
    pub mcp_request_count: u64,
    pub search_count: u64,
}

/// The decrypted, parsed knowledge data plus its search runtime. For plaintext
/// Orbs this is built at startup; for asset-encrypted Orbs it is built on the
/// first successful unlock (Phase 4). Held behind `OnceLock` so that, once
/// published, every read is lock-free — keeping it off the search hot path
/// (plan §4.2, EF1).
pub struct LoadedKnowledge {
    pub manifest: OrbManifest,
    pub documents: Vec<Document>,
    pub chunks: Vec<Chunk>,
    pub search: SearchRuntime,
}

/// Result of loading an Orb's bundle: its security policy plus its knowledge.
/// Replaces the previous 4-tuple return shape (plan §4.2, E1).
pub struct LoadedOrb {
    pub security: SecurityConfig,
    pub knowledge: LoadedKnowledge,
}

pub struct OrbState {
    /// Access-password gate + process-global unlock flag. See `security.rs`.
    pub security: SecurityState,
    /// Knowledge data. Always populated at startup for plaintext Orbs; populated
    /// on unlock for encrypted Orbs (Phase 4). Reads go through [`knowledge`] /
    /// [`knowledge_opt`], never the field directly.
    knowledge: OnceLock<LoadedKnowledge>,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        security: SecurityConfig,
        knowledge: Option<LoadedKnowledge>,
        #[cfg(feature = "vector-embedder")] model_manager: Arc<ModelManager>,
        #[cfg(feature = "vector-embedder")] embedder_slot: Arc<EmbedderSlot>,
        startup_mode: String,
        orb_binary_path: Option<String>,
        gui_url: Option<String>,
    ) -> SharedState {
        let knowledge_slot = OnceLock::new();
        if let Some(k) = knowledge {
            // Infallible: the slot is fresh.
            let _ = knowledge_slot.set(k);
        }
        Arc::new(OrbState {
            security: SecurityState::new(security),
            knowledge: knowledge_slot,
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

    /// Knowledge accessor for gated paths. Callers reach here only after the
    /// unlock gate has passed, at which point the data is guaranteed loaded.
    ///
    /// Panics if called while still locked/encrypted — that is a routing bug
    /// (a protected handler ran without the gate). Always-allowed handlers
    /// (e.g. MCP `initialize`) must use [`knowledge_opt`] instead.
    pub fn knowledge(&self) -> &LoadedKnowledge {
        self.knowledge
            .get()
            .expect("knowledge accessed before unlock — missing auth gate")
    }

    /// Non-panicking accessor for always-allowed paths that must tolerate the
    /// not-yet-unlocked (encrypted) state.
    pub fn knowledge_opt(&self) -> Option<&LoadedKnowledge> {
        self.knowledge.get()
    }

    /// Publish freshly decrypted knowledge after an unlock (Phase 4). Returns
    /// `false` if it was already set (a benign race — first writer wins).
    #[allow(dead_code)]
    pub fn set_knowledge(&self, knowledge: LoadedKnowledge) -> bool {
        self.knowledge.set(knowledge).is_ok()
    }
}
