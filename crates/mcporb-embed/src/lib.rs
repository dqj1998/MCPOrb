//! Embedded query embedder for MCP Orb.
//!
//! Loads a fixed ONNX sentence-transformer model via tract and produces
//! 384-dim L2-normalized query vectors locally, so MCP clients can use
//! `method=vector` without supplying `query_vector` themselves.
//!
//! See `plans/vector-search-spec.md` for the full design.

pub mod downloader;
pub mod embedder;
pub mod model;

pub use downloader::ModelManager;
pub use embedder::{embed, empty_slot, EmbedderSlot, TractEmbedder};
pub use model::{MAX_SEQ_LEN, MODEL_DIM, MODEL_ID, MODEL_TAR_SHA256, MODEL_URLS};
