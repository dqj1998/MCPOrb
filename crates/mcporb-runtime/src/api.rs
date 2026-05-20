use axum::{
    extract::State,
    response::Json,
};
use mcporb_runtime_core::{SearchMethodRequest, SearchRequest as RuntimeSearchRequest};
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
        "selected_retrieval_plan": state.manifest.selected_retrieval_plan.to_string(),
        "enabled_capabilities": state.search.capabilities().iter().map(|c| format!("{c:?}").to_lowercase()).collect::<Vec<_>>(),
        "available_methods": state.search.available_method_names(),
        "planning_rationale": state.manifest.planning_rationale,
        "source_documents": state.manifest.source_documents,
        "mcp_protocol_version": state.manifest.mcp_protocol_version,
        "orb_binary_path": state.orb_binary_path,
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
    pub method: Option<String>,
    pub query_vector: Option<Vec<f32>>,
}

pub async fn post_search(
    State(state): State<SharedState>,
    Json(req): Json<SearchRequest>,
) -> Json<Value> {
    let top_k = req.top_k.unwrap_or(5);
    let method_name = req.method.as_deref().unwrap_or("auto");
    {
        let mut metrics = state.metrics.write().await;
        metrics.search_count += 1;
    }

    let available_methods = state.search.available_method_names();
    if !available_methods.iter().any(|value| *value == method_name) {
        return Json(json!({
            "query": req.query,
            "error": format!("Unsupported method: {method_name}"),
            "available_methods": available_methods,
        }));
    }

    match state.search.search(&RuntimeSearchRequest {
        query: req.query.clone(),
        top_k,
        method: SearchMethodRequest::from_str(method_name),
        query_vector: req.query_vector.clone(),
        explain: false,
    }) {
        Ok(response) => {
            let hits: Vec<Value> = response.hits.iter().filter_map(|result| {
                state.chunks.get(result.chunk_id as usize).map(|chunk| json!({
                    "chunk_id": chunk.id,
                    "score": result.score,
                    "method": result.method.to_string(),
                    "page": chunk.page,
                    "section_id": chunk.section_id,
                    "text": chunk.text,
                    "token_count": chunk.token_count,
                }))
            }).collect();

            Json(json!({
                "query": req.query,
                "method": method_name,
                "active_plan": response.active_plan.to_string(),
                "hits": hits,
                "total": hits.len()
            }))
        }
        Err(error) => Json(json!({
            "query": req.query,
            "method": method_name,
            "error": error.to_string(),
        })),
    }
}
