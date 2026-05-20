use std::time::Instant;

use anyhow::{anyhow, bail, Result};

use crate::format::{Bm25Index, Capability, RetrievalPlanKind, TfIdfIndex, TrigramIndex};
use crate::search::{rrf_fuse, SearchMethod, SearchMethodRequest, SearchResult};

pub struct SearchRuntime {
    pub bm25: Bm25Index,
    #[cfg(feature = "tfidf")]
    pub tfidf: Option<TfIdfIndex>,
    #[cfg(feature = "trigram")]
    pub trigram: Option<TrigramIndex>,
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
            SearchMethodRequest::FlatVector => bail!("method=vector not available in this Orb"),
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

    fn run_hybrid(&self, request: &SearchRequest) -> Result<SearchResponse> {
        if !self.supports_hybrid() {
            bail!("method=hybrid not available in this Orb");
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