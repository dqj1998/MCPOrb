use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMethod {
    Bm25,
    TfIdf,
    FlatVector,
    Hnsw,
    Trigram,
    Hybrid,
}

impl std::fmt::Display for SearchMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchMethod::Bm25 => write!(f, "BM25"),
            SearchMethod::TfIdf => write!(f, "TF-IDF"),
            SearchMethod::FlatVector => write!(f, "Vector"),
            SearchMethod::Hnsw => write!(f, "HNSW"),
            SearchMethod::Trigram => write!(f, "Trigram"),
            SearchMethod::Hybrid => write!(f, "Hybrid"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMethodRequest {
    Auto,
    Bm25,
    TfIdf,
    FlatVector,
    Trigram,
    Hybrid,
}

impl SearchMethodRequest {
    pub fn from_str(value: &str) -> Self {
        match value {
            "bm25" => Self::Bm25,
            "tfidf" => Self::TfIdf,
            "vector" => Self::FlatVector,
            "trigram" => Self::Trigram,
            "hybrid" => Self::Hybrid,
            _ => Self::Auto,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Bm25 => "bm25",
            Self::TfIdf => "tfidf",
            Self::FlatVector => "vector",
            Self::Trigram => "trigram",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub chunk_id: u32,
    pub score: f32,
    pub method: SearchMethod,
}

pub fn rrf_fuse(ranked_lists: Vec<Vec<SearchResult>>, top_k: usize) -> Vec<SearchResult> {
    const RRF_K: f32 = 60.0;

    let mut fused_scores: HashMap<u32, f32> = HashMap::new();
    for ranked_list in ranked_lists {
        for (rank, result) in ranked_list.into_iter().enumerate() {
            let contribution = 1.0 / (RRF_K + rank as f32 + 1.0);
            *fused_scores.entry(result.chunk_id).or_insert(0.0) += contribution;
        }
    }

    let mut fused: Vec<SearchResult> = fused_scores
        .into_iter()
        .map(|(chunk_id, score)| SearchResult {
            chunk_id,
            score,
            method: SearchMethod::Hybrid,
        })
        .collect();

    fused.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    fused.truncate(top_k);
    fused
}
