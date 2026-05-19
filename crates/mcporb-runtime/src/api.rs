use axum::{
    extract::State,
    response::Json,
};
use mcporb_runtime_core::bm25_search;
use serde_json::{json, Value};
use crate::state::SharedState;

pub async fn get_manifest(State(state): State<SharedState>) -> Json<Value> {
    Json(json!({
        "name": state.manifest.name,
        "version": state.manifest.version,
        "description": state.manifest.description,
        "build_time": state.manifest.build_time,
        "chunk_count": state.manifest.chunk_count,
        "orb_format_version": state.manifest.orb_format_version,
        "startup_mode": state.startup_mode,
    }))
}

pub async fn get_documents(State(state): State<SharedState>) -> Json<Value> {
    let docs: Vec<Value> = state.documents.iter().map(|d| json!({
        "id": d.id,
        "title": d.title,
        "source_path": d.source_path,
        "page_count": d.page_count,
        "section_count": d.sections.len(),
    })).collect();
    Json(json!({ "documents": docs }))
}

pub async fn get_metrics(State(state): State<SharedState>) -> Json<Value> {
    let metrics = state.metrics.read().await;
    Json(json!({
        "mcp_request_count": metrics.mcp_request_count,
        "search_count": metrics.search_count,
        "startup_mode": state.startup_mode,
    }))
}

#[derive(serde::Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub top_k: Option<usize>,
}

pub async fn post_search(
    State(state): State<SharedState>,
    Json(req): Json<SearchRequest>,
) -> Json<Value> {
    let top_k = req.top_k.unwrap_or(5);
    {
        let mut metrics = state.metrics.write().await;
        metrics.search_count += 1;
    }

    let results = bm25_search(&state.index, &req.query, top_k);

    let hits: Vec<Value> = results.iter().filter_map(|(chunk_id, score)| {
        state.chunks.get(*chunk_id as usize).map(|chunk| json!({
            "chunk_id": chunk.id,
            "score": score,
            "page": chunk.page,
            "section_id": chunk.section_id,
            "text": &chunk.text[..chunk.text.len().min(300)],
            "token_count": chunk.token_count,
        }))
    }).collect();

    Json(json!({ "query": req.query, "hits": hits, "total": hits.len() }))
}
