use std::time::Instant;

use anyhow::{anyhow, bail, Result};

use crate::format::{
    Bm25Index, Capability, FlatVectorIndex, HnswIndex, RetrievalPlanKind, TfIdfIndex,
    TrigramIndex,
};
use crate::search::{rrf_fuse, SearchMethod, SearchMethodRequest, SearchResult};

pub enum DenseRuntime {
    None,
    #[cfg(feature = "vector")]
    Flat { store: FlatVectorIndex },
    #[cfg(feature = "hnsw")]
    Hnsw {
        store: FlatVectorIndex,
        ann: crate::vector::DenseHnswMap,
    },
}

impl DenseRuntime {
    pub fn from_assets(flat: Option<FlatVectorIndex>, hnsw: Option<HnswIndex>) -> Result<Self> {
        match (flat, hnsw) {
            (None, None) => Ok(Self::None),
            (None, Some(_)) => bail!("HNSW metadata present without vector_store.postcard"),
            (Some(store), Some(metadata)) => {
                #[cfg(feature = "hnsw")]
                {
                    return Ok(Self::Hnsw {
                        ann: crate::vector::build_hnsw(
                            &store,
                            metadata.ef_construction,
                            metadata.ef_search,
                        ),
                        store,
                    });
                }

                #[cfg(all(feature = "vector", not(feature = "hnsw")))]
                {
                    let _ = metadata;
                    return Ok(Self::Flat { store });
                }

                #[cfg(not(feature = "vector"))]
                {
                    let _ = store;
                    let _ = metadata;
                    bail!("dense runtime assets are present but vector support is not compiled in")
                }
            }
            (Some(store), None) => {
                #[cfg(feature = "vector")]
                {
                    return Ok(Self::Flat { store });
                }

                #[cfg(not(feature = "vector"))]
                {
                    let _ = store;
                    bail!("vector_store.postcard is present but vector support is not compiled in")
                }
            }
        }
    }

    pub fn has_dense(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn uses_hnsw(&self) -> bool {
        #[cfg(feature = "hnsw")]
        {
            matches!(self, Self::Hnsw { .. })
        }
        #[cfg(not(feature = "hnsw"))]
        {
            false
        }
    }

    fn store(&self) -> Option<&FlatVectorIndex> {
        match self {
            Self::None => None,
            #[cfg(feature = "vector")]
            Self::Flat { store } => Some(store),
            #[cfg(feature = "hnsw")]
            Self::Hnsw { store, .. } => Some(store),
        }
    }
}

pub struct SearchRuntime {
    pub bm25: Bm25Index,
    #[cfg(feature = "tfidf")]
    pub tfidf: Option<TfIdfIndex>,
    #[cfg(feature = "trigram")]
    pub trigram: Option<TrigramIndex>,
    pub dense: DenseRuntime,
    pub dense_tier: RetrievalPlanKind,
}

pub struct SearchRequest {
    pub query: String,
    pub top_k: usize,
    pub method: SearchMethodRequest,
    pub query_vector: Option<Vec<f32>>,
    pub explain: bool,
}

#[derive(Debug, Clone)]
pub struct SearchStageTrace {
    pub stage: String,
    pub input_count: usize,
    pub output_count: usize,
    pub latency_us: u64,
    pub notes: Vec<String>,
}

#[derive(Debug)]
pub struct SearchResponse {
    pub active_plan: RetrievalPlanKind,
    pub hits: Vec<SearchResult>,
    pub traces: Vec<SearchStageTrace>,
    pub fallback_triggered: bool,
}

impl SearchRuntime {
    pub fn capabilities(&self) -> Vec<Capability> {
        let mut capabilities = vec![Capability::Bm25];

        #[cfg(feature = "tfidf")]
        if self.tfidf.is_some() {
            capabilities.push(Capability::TfIdf);
        }

        #[cfg(feature = "trigram")]
        if self.trigram.is_some() {
            capabilities.push(Capability::Trigram);
        }

        if self.dense.has_dense() {
            capabilities.push(Capability::FlatVector);
        }
        if self.dense.uses_hnsw() {
            capabilities.push(Capability::Hnsw);
        }

        capabilities
    }

    pub fn active_plan(&self) -> RetrievalPlanKind {
        self.dense_tier.clone()
    }

    pub fn supports_hybrid(&self) -> bool {
        let mut other_rankers = 0;

        #[cfg(feature = "tfidf")]
        if self.tfidf.is_some() {
            other_rankers += 1;
        }

        #[cfg(feature = "trigram")]
        if self.trigram.is_some() {
            other_rankers += 1;
        }

        if self.dense.has_dense() {
            other_rankers += 1;
        }

        other_rankers > 0
    }

    pub fn available_method_names(&self) -> Vec<&'static str> {
        let mut methods = vec!["auto", "bm25"];

        #[cfg(feature = "tfidf")]
        if self.tfidf.is_some() {
            methods.push("tfidf");
        }

        #[cfg(feature = "trigram")]
        if self.trigram.is_some() {
            methods.push("trigram");
        }

        if self.dense.has_dense() {
            methods.push("vector");
        }

        if self.supports_hybrid() {
            methods.push("hybrid");
        }

        methods
    }

    pub fn search(&self, request: &SearchRequest) -> Result<SearchResponse> {
        match request.method {
            SearchMethodRequest::Auto => self.run_auto(request),
            SearchMethodRequest::Bm25 => self.run_single(SearchMethod::Bm25, request),
            SearchMethodRequest::TfIdf => self.run_tfidf_only(request),
            SearchMethodRequest::Trigram => self.run_trigram_only(request),
            SearchMethodRequest::Hybrid => self.run_hybrid(request),
            SearchMethodRequest::FlatVector => self.run_dense_only(request),
        }
    }

    fn run_auto(&self, request: &SearchRequest) -> Result<SearchResponse> {
        let mut traces = Vec::new();
        let mut ranked_lists = vec![self.run_bm25_raw(request, &mut traces)];

        #[cfg(feature = "tfidf")]
        if self.tfidf.is_some() {
            ranked_lists.push(self.run_tfidf_raw(request, &mut traces)?);
        }

        #[cfg(feature = "trigram")]
        if self.trigram.is_some() {
            ranked_lists.push(self.run_trigram_raw(request, &mut traces)?);
        }

        if self.dense.has_dense() {
            if request.query_vector.is_some() {
                ranked_lists.push(self.run_dense_raw(request, &mut traces)?);
            } else {
                push_trace(
                    request,
                    &mut traces,
                    "vector",
                    0,
                    Instant::now(),
                    vec!["dense tier available but query_vector was omitted; skipped in auto mode".to_string()],
                );
            }
        }

        let hits = if ranked_lists.len() == 1 {
            ranked_lists.pop().unwrap_or_default()
        } else {
            rrf_fuse(ranked_lists, request.top_k)
        };

        Ok(SearchResponse {
            active_plan: self.active_plan(),
            hits,
            traces,
            fallback_triggered: false,
        })
    }

    fn run_single(&self, method: SearchMethod, request: &SearchRequest) -> Result<SearchResponse> {
        let mut traces = Vec::new();
        let hits = match method {
            SearchMethod::Bm25 => self.run_bm25_raw(request, &mut traces),
            _ => return Err(anyhow!("unsupported single-method dispatch")),
        };

        Ok(SearchResponse {
            active_plan: self.active_plan(),
            hits,
            traces,
            fallback_triggered: false,
        })
    }

    fn run_tfidf_only(&self, request: &SearchRequest) -> Result<SearchResponse> {
        let mut traces = Vec::new();
        let hits = self.run_tfidf_raw(request, &mut traces)?;
        Ok(SearchResponse {
            active_plan: self.active_plan(),
            hits,
            traces,
            fallback_triggered: false,
        })
    }

    fn run_trigram_only(&self, request: &SearchRequest) -> Result<SearchResponse> {
        let mut traces = Vec::new();
        let hits = self.run_trigram_raw(request, &mut traces)?;
        Ok(SearchResponse {
            active_plan: self.active_plan(),
            hits,
            traces,
            fallback_triggered: false,
        })
    }

    fn run_dense_only(&self, request: &SearchRequest) -> Result<SearchResponse> {
        let mut traces = Vec::new();
        let hits = self.run_dense_raw(request, &mut traces)?;
        Ok(SearchResponse {
            active_plan: self.active_plan(),
            hits,
            traces,
            fallback_triggered: false,
        })
    }

    fn run_hybrid(&self, request: &SearchRequest) -> Result<SearchResponse> {
        if !self.supports_hybrid() {
            bail!("method=hybrid not available in this Orb");
        }

        if self.dense.has_dense() && request.query_vector.is_none() {
            bail!("method=hybrid requires query_vector when dense retrieval is enabled");
        }

        let mut traces = Vec::new();
        let mut ranked_lists = vec![self.run_bm25_raw(request, &mut traces)];

        #[cfg(feature = "tfidf")]
        if self.tfidf.is_some() {
            ranked_lists.push(self.run_tfidf_raw(request, &mut traces)?);
        }

        #[cfg(feature = "trigram")]
        if self.trigram.is_some() {
            ranked_lists.push(self.run_trigram_raw(request, &mut traces)?);
        }

        if self.dense.has_dense() {
            ranked_lists.push(self.run_dense_raw(request, &mut traces)?);
        }

        Ok(SearchResponse {
            active_plan: self.active_plan(),
            hits: rrf_fuse(ranked_lists, request.top_k),
            traces,
            fallback_triggered: false,
        })
    }

    fn run_bm25_raw(
        &self,
        request: &SearchRequest,
        traces: &mut Vec<SearchStageTrace>,
    ) -> Vec<SearchResult> {
        let started_at = Instant::now();
        let hits = crate::bm25::search(&self.bm25, &request.query, request.top_k)
            .into_iter()
            .map(|(chunk_id, score)| SearchResult {
                chunk_id,
                score,
                method: SearchMethod::Bm25,
            })
            .collect::<Vec<_>>();
        push_trace(request, traces, "bm25", hits.len(), started_at, Vec::new());
        hits
    }

    fn run_tfidf_raw(
        &self,
        request: &SearchRequest,
        traces: &mut Vec<SearchStageTrace>,
    ) -> Result<Vec<SearchResult>> {
        #[cfg(feature = "tfidf")]
        {
            let index = self
                .tfidf
                .as_ref()
                .ok_or_else(|| anyhow!("method=tfidf not available in this Orb"))?;
            let started_at = Instant::now();
            let hits = crate::tfidf::search(index, &request.query, request.top_k)
                .into_iter()
                .map(|(chunk_id, score)| SearchResult {
                    chunk_id,
                    score,
                    method: SearchMethod::TfIdf,
                })
                .collect::<Vec<_>>();
            push_trace(request, traces, "tfidf", hits.len(), started_at, Vec::new());
            Ok(hits)
        }

        #[cfg(not(feature = "tfidf"))]
        {
            let _ = request;
            let _ = traces;
            bail!("method=tfidf not compiled into this runtime")
        }
    }

    fn run_trigram_raw(
        &self,
        request: &SearchRequest,
        traces: &mut Vec<SearchStageTrace>,
    ) -> Result<Vec<SearchResult>> {
        #[cfg(feature = "trigram")]
        {
            let index = self
                .trigram
                .as_ref()
                .ok_or_else(|| anyhow!("method=trigram not available in this Orb"))?;
            let started_at = Instant::now();
            let hits = crate::trigram::search(index, &request.query, request.top_k)
                .into_iter()
                .map(|(chunk_id, score)| SearchResult {
                    chunk_id,
                    score,
                    method: SearchMethod::Trigram,
                })
                .collect::<Vec<_>>();
            push_trace(request, traces, "trigram", hits.len(), started_at, Vec::new());
            Ok(hits)
        }

        #[cfg(not(feature = "trigram"))]
        {
            let _ = request;
            let _ = traces;
            bail!("method=trigram not compiled into this runtime")
        }
    }

    fn run_dense_raw(
        &self,
        request: &SearchRequest,
        traces: &mut Vec<SearchStageTrace>,
    ) -> Result<Vec<SearchResult>> {
        #[cfg(feature = "vector")]
        {
            let query_vector = request
                .query_vector
                .as_ref()
                .ok_or_else(|| anyhow!("method=vector requires query_vector"))?;
            let store = self
                .dense
                .store()
                .ok_or_else(|| anyhow!("method=vector not available in this Orb"))?;
            crate::vector::validate_query_vector(store, query_vector)?;

            let started_at = Instant::now();
            let hits = match &self.dense {
                DenseRuntime::None => bail!("method=vector not available in this Orb"),
                DenseRuntime::Flat { store } => crate::vector::search(store, query_vector, request.top_k)
                    .into_iter()
                    .map(|(chunk_id, score)| SearchResult {
                        chunk_id,
                        score,
                        method: SearchMethod::FlatVector,
                    })
                    .collect::<Vec<_>>(),
                #[cfg(feature = "hnsw")]
                DenseRuntime::Hnsw { ann, .. } => crate::vector::search_hnsw(ann, query_vector, request.top_k)
                    .into_iter()
                    .map(|(chunk_id, score)| SearchResult {
                        chunk_id,
                        score,
                        method: SearchMethod::Hnsw,
                    })
                    .collect::<Vec<_>>(),
            };
            let stage = if self.dense.uses_hnsw() { "vector_hnsw" } else { "vector_flat" };
            push_trace(request, traces, stage, hits.len(), started_at, Vec::new());
            Ok(hits)
        }

        #[cfg(not(feature = "vector"))]
        {
            let _ = request;
            let _ = traces;
            bail!("method=vector not compiled into this runtime")
        }
    }
}

fn push_trace(
    request: &SearchRequest,
    traces: &mut Vec<SearchStageTrace>,
    stage: &str,
    output_count: usize,
    started_at: Instant,
    notes: Vec<String>,
) {
    if request.explain {
        traces.push(SearchStageTrace {
            stage: stage.to_string(),
            input_count: request.top_k,
            output_count,
            latency_us: started_at.elapsed().as_micros() as u64,
            notes,
        });
    }
}