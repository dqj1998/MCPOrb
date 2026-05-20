pub mod bm25;
pub mod format;
#[cfg(feature = "vector")]
pub mod vector;
#[cfg(feature = "tfidf")]
pub mod tfidf;
#[cfg(feature = "trigram")]
pub mod trigram;
pub mod search;
pub mod runtime;

pub use bm25::{build_index as build_bm25_index, search as bm25_search, tokenize};
pub use format::{Bm25Index, Chunk, Document, FlatVectorIndex, HnswIndex, OrbManifest, Section, TfIdfIndex, TrigramIndex};
pub use runtime::{DenseRuntime, SearchRequest, SearchResponse, SearchRuntime, SearchStageTrace};
pub use search::{SearchMethod, SearchMethodRequest, SearchResult, rrf_fuse};
#[cfg(feature = "tfidf")]
pub use tfidf::{build_index as build_tfidf_index, search as tfidf_search};
#[cfg(feature = "trigram")]
pub use trigram::{build_index as build_trigram_index, search as trigram_search};
#[cfg(feature = "vector")]
pub use vector::search as vector_search;
