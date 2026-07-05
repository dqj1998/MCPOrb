#[cfg(feature = "vector-embedder")]
use mcporb_embed::{EmbedderSlot, ModelManager};
use mcporb_runtime_core::{Chunk, Document, OrbManifest, SearchRuntime};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::RwLock;

use crate::security::{SecurityConfig, SecurityState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaEntry {
    pub timestamp: String,
    pub query: String,
    pub response_preview: String,
    pub method: String,
    pub num_results: usize,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub stdio_requests: u64,
    pub http_requests: u64,
    pub total_searches: u64,
    pub qa_history: Vec<QaEntry>,
    pub startup_mode: String,
}

impl MetricsSnapshot {
    pub fn from(metrics: &Metrics, startup_mode: &str) -> Self {
        Self {
            total_requests: metrics.total_requests,
            stdio_requests: metrics.stdio_requests,
            http_requests: metrics.http_requests,
            total_searches: metrics.total_searches,
            qa_history: metrics.qa_history.clone(),
            startup_mode: startup_mode.to_string(),
        }
    }
}

/// Max Q&A entries kept in memory ring buffer.
const MAX_QA_ENTRIES: usize = 500;

#[derive(Debug, Serialize, Deserialize)]
pub struct Metrics {
    pub total_requests: u64,
    pub stdio_requests: u64,
    pub http_requests: u64,
    pub total_searches: u64,
    pub qa_history: Vec<QaEntry>,
    #[serde(skip)]
    file_path: Option<PathBuf>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            stdio_requests: 0,
            http_requests: 0,
            total_searches: 0,
            qa_history: Vec::new(),
            file_path: None,
        }
    }
}

impl Metrics {
    pub fn with_file(path: PathBuf) -> Self {
        let mut m = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Metrics>(&s).ok())
            .unwrap_or_default();
        m.file_path = Some(path);
        m
    }

    fn sync_to_disk(&self) {
        if let Some(ref path) = self.file_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn record_search(&mut self, qa: QaEntry) {
        self.total_searches += 1;
        self.total_requests += 1;
        match qa.transport.as_str() {
            "stdio" => self.stdio_requests += 1,
            "http" => self.http_requests += 1,
            _ => {}
        }
        self.qa_history.push(qa);
        if self.qa_history.len() > MAX_QA_ENTRIES {
            self.qa_history.remove(0);
        }
        self.sync_to_disk();
    }
}

/// Returns an ISO-8601 UTC timestamp string.
///
/// Format example: `2026-07-05T13:40:00Z`
/// The frontend converts to local timezone for display.
pub fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
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

/// What a loader produced for an Orb's assets: either plaintext knowledge ready
/// to serve, or the still-encrypted payload awaiting an unlock (plan §4.2).
pub enum LoadedAssets {
    Plain(LoadedKnowledge),
    /// Ciphertext bytes of `orb_assets.enc`; decrypted on unlock (Phase 4).
    Encrypted(Vec<u8>),
}

/// Result of loading an Orb's bundle: its security policy plus its assets.
/// Replaces the previous 4-tuple return shape (plan §4.2, E1).
pub struct LoadedOrb {
    pub security: SecurityConfig,
    pub assets: LoadedAssets,
}

pub struct OrbState {
    /// Access-password gate + process-global unlock flag. See `security.rs`.
    pub security: SecurityState,
    /// Knowledge data. Always populated at startup for plaintext Orbs; populated
    /// on unlock for encrypted Orbs (Phase 4). Reads go through [`knowledge`] /
    /// [`knowledge_opt`], never the field directly.
    knowledge: OnceLock<LoadedKnowledge>,
    /// Ciphertext of `orb_assets.enc` for an encrypted Orb, held until unlock
    /// decrypts it into `knowledge` and clears this. `None` for plaintext Orbs.
    encrypted_blob: Mutex<Option<Vec<u8>>>,
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
        assets: LoadedAssets,
        #[cfg(feature = "vector-embedder")] model_manager: Arc<ModelManager>,
        #[cfg(feature = "vector-embedder")] embedder_slot: Arc<EmbedderSlot>,
        startup_mode: String,
        orb_binary_path: Option<String>,
        gui_url: Option<String>,
        metrics_file: Option<PathBuf>,
    ) -> SharedState {
        let knowledge_slot = OnceLock::new();
        let mut encrypted = None;
        match assets {
            // Plaintext Orb: publish knowledge now (infallible — slot is fresh).
            LoadedAssets::Plain(k) => {
                let _ = knowledge_slot.set(k);
            }
            // Encrypted Orb: hold ciphertext until unlock decrypts it.
            LoadedAssets::Encrypted(blob) => encrypted = Some(blob),
        }
        Arc::new(OrbState {
            security: SecurityState::new(security),
            knowledge: knowledge_slot,
            encrypted_blob: Mutex::new(encrypted),
            #[cfg(feature = "vector-embedder")]
            model_manager,
            #[cfg(feature = "vector-embedder")]
            embedder_slot,
            metrics: RwLock::new(
                metrics_file.map(Metrics::with_file).unwrap_or_default(),
            ),
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
    pub fn set_knowledge(&self, knowledge: LoadedKnowledge) -> bool {
        self.knowledge.set(knowledge).is_ok()
    }

    /// Clone the held ciphertext for an encrypted Orb (the rare unlock path).
    /// `None` once decrypted/cleared or for a plaintext Orb.
    pub fn encrypted_blob_clone(&self) -> Option<Vec<u8>> {
        self.encrypted_blob.lock().expect("encrypted_blob poisoned").clone()
    }

    /// Drop the ciphertext once it has been decrypted and published.
    pub fn clear_encrypted_blob(&self) {
        *self.encrypted_blob.lock().expect("encrypted_blob poisoned") = None;
    }
}
