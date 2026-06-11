use std::io::{BufRead, Write};
#[cfg(feature = "vector-embedder")]
use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
#[cfg(feature = "vector-embedder")]
use mcporb_embed::TractEmbedder;
#[cfg(feature = "vector-embedder")]
use mcporb_runtime_core::format::Capability;
use mcporb_runtime_core::{SearchMethodRequest, SearchRequest};
use serde_json::{json, Value};

use crate::state::SharedState;

/// Build a human-readable description of the `method` parameter for `tools/list`,
/// based on which methods are actually available in this Orb at runtime.
/// This description is shown to the LLM so it can choose the right method.
fn build_method_description(methods: &[&str]) -> String {
    let mut parts: Vec<&str> = vec!["Search method (default: auto)."];
    if methods.contains(&"auto") {
        parts.push("'auto': automatically picks the best available method(s).");
    }
    if methods.contains(&"bm25") {
        parts.push("'bm25': exact keyword match, best for precise term lookup.");
    }
    if methods.contains(&"tfidf") {
        parts.push("'tfidf': term-frequency ranking, good for topical relevance.");
    }
    if methods.contains(&"trigram") {
        parts.push("'trigram': fuzzy/typo-tolerant character-level match.");
    }
    if methods.contains(&"vector") {
        parts.push(
            "'vector': semantic similarity search, best for conceptual or paraphrase queries.",
        );
    }
    if methods.contains(&"hybrid") {
        parts.push("'hybrid': fuses all available rankers via RRF, recommended for mixed queries.");
    }
    parts.join(" ")
}

pub async fn handle_json_rpc_request(
    state: &SharedState,
    request: Value,
) -> anyhow::Result<Option<Value>> {
    if let Some(batch) = request.as_array() {
        if batch.is_empty() {
            return Ok(Some(json_rpc_error(Value::Null, -32600, "Invalid Request")));
        }

        let mut responses = Vec::new();
        for item in batch {
            if let Some(response) = handle_single_json_rpc_request(state, item.clone()).await? {
                responses.push(response);
            }
        }

        return if responses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Value::Array(responses)))
        };
    }

    handle_single_json_rpc_request(state, request).await
}

async fn handle_single_json_rpc_request(
    state: &SharedState,
    request: Value,
) -> anyhow::Result<Option<Value>> {
    if !request.is_object() {
        return Ok(Some(json_rpc_error(Value::Null, -32600, "Invalid Request")));
    }

    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    if matches!(method, "notifications/initialized" | "$/cancelRequest") {
        return Ok(None);
    }

    {
        let mut metrics = state.metrics.write().await;
        metrics.mcp_request_count += 1;
    }

    let response = match method {
        // Always allowed — even when locked. Must not read knowledge directly:
        // an asset-encrypted Orb has no manifest until unlocked (plan §4.4).
        "initialize" => {
            let (name, version, description) = match state.knowledge_opt() {
                Some(k) => (
                    k.manifest.name.clone(),
                    k.manifest.version.clone(),
                    k.manifest.description.clone(),
                ),
                None => ("MCPOrb".to_string(), String::new(), String::new()),
            };
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {}, "resources": {} },
                    "serverInfo": { "name": name, "version": version, "description": description }
                }
            })
        }
        // Locked Orbs expose only the unlock affordances (plan §3.2).
        "tools/list" => {
            let tools = if state.security.is_unlocked() {
                let k = state.knowledge();
                json!([
                    {
                        "name": "search_knowledge",
                        "description": format!("Search the {} knowledge base", k.manifest.name),
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Search query" },
                                "top_k": { "type": "integer", "description": "Number of results (default: 5)" },
                                "method": {
                                    "type": "string",
                                    "description": build_method_description(&k.search.available_method_names()),
                                    "enum": k.search.available_method_names()
                                }
                            },
                            "required": ["query"]
                        }
                    },
                    get_web_ui_url_tool_def()
                ])
            } else {
                json!([unlock_orb_tool_def(), get_web_ui_url_tool_def()])
            };
            json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools } })
        }
        "tools/call" => handle_tool_call(state, id, request).await?,
        "resources/list" => {
            if state.security.require_unlocked().is_err() {
                locked_error(id)
            } else {
                let resources: Vec<Value> = state
                    .knowledge()
                    .documents
                    .iter()
                    .map(|document| {
                        json!({
                            "uri": format!("orb://documents/{}", document.id),
                            "name": document.title,
                            "mimeType": "text/plain"
                        })
                    })
                    .collect();
                json!({ "jsonrpc": "2.0", "id": id, "result": { "resources": resources } })
            }
        }
        "resources/read" => {
            if state.security.require_unlocked().is_err() {
                locked_error(id)
            } else {
                handle_resource_read(state, id, request)
            }
        }
        _ => json_rpc_error(id, -32601, &format!("Method not found: {method}")),
    };

    Ok(Some(response))
}

/// `tools/list` entry for the password-unlock tool (shown only while locked).
fn unlock_orb_tool_def() -> Value {
    json!({
        "name": "unlock_orb",
        "description": "This Orb is locked. Open the Web UI (get_web_ui_url) to unlock without sharing the password here, or pass the password to unlock now.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "password": { "type": "string", "description": "Password for this local Orb" }
            },
            "required": ["password"]
        }
    })
}

fn get_web_ui_url_tool_def() -> Value {
    json!({
        "name": "get_web_ui_url",
        "description": "Get the local Web UI URL for this Orb when GUI mode is enabled",
        "inputSchema": { "type": "object", "properties": {} }
    })
}

/// Standard locked response. References the Web UI unlock path (plan §4.4).
fn locked_error(id: Value) -> Value {
    json_rpc_error(
        id,
        -32001,
        "Orb is locked. Open the Web UI to unlock (see get_web_ui_url), or call unlock_orb with the password.",
    )
}

async fn handle_tool_call(state: &SharedState, id: Value, request: Value) -> anyhow::Result<Value> {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    // Always-allowed: unlock the process with a password (fallback path; the
    // primary UX is browser unlock via the shared Web UI — plan §3.2).
    if tool_name == "unlock_orb" {
        return Ok(handle_unlock_orb(state, id, &params).await);
    }

    if tool_name == "search_knowledge" {
        // Gate: knowledge is only readable once unlocked (plan §4.4).
        if state.security.require_unlocked().is_err() {
            return Ok(locked_error(id));
        }
        let k = state.knowledge();
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        let query = args
            .get("query")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let top_k = args
            .get("top_k")
            .and_then(|value| value.as_u64())
            .unwrap_or(5) as usize;
        let method_name = args
            .get("method")
            .and_then(|value| value.as_str())
            .unwrap_or("auto");
        let query_vector = args
            .get("query_vector")
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_f64())
                    .map(|value| value as f32)
                    .collect::<Vec<_>>()
            });

        {
            let mut metrics = state.metrics.write().await;
            metrics.search_count += 1;
        }

        let available_methods = k.search.available_method_names();
        if !available_methods.iter().any(|value| *value == method_name) {
            return Ok(json_rpc_error(
                id,
                -32602,
                &format!(
                    "Unsupported method: {method_name}. Available methods: {}",
                    available_methods.join(", ")
                ),
            ));
        }

        let requested_method = SearchMethodRequest::from_str(method_name);
        return match auto_fill_query_vector(state, requested_method, &query, query_vector).await {
            Err(msg) => Ok(json_rpc_error(id, -32602, &msg)),
            Ok(prepared) => match k.search.search(&SearchRequest {
                query: query.clone(),
                top_k,
                method: prepared.method,
                query_vector: prepared.query_vector,
                explain: false,
            }) {
                Ok(result) => {
                    let content: Vec<Value> = result
                        .hits
                        .iter()
                        .filter_map(|hit| {
                            k.chunks.get(hit.chunk_id as usize).map(|chunk| {
                                let preview = &chunk.text[..chunk.text.len().min(2000)];
                                json!({
                                    "type": "text",
                                    "text": format!("[{} Score: {:.3}] Page {:?}\n{}", hit.method, hit.score, chunk.page, preview)
                                })
                            })
                        })
                        .collect();
                    let mut result_obj = serde_json::Map::new();
                    result_obj.insert("content".to_string(), json!(content));
                    result_obj.insert(
                        "active_plan".to_string(),
                        json!(result.active_plan.to_string()),
                    );
                    if !prepared.metadata.is_empty() {
                        let mut meta_obj = serde_json::Map::new();
                        for (k, v) in &prepared.metadata {
                            meta_obj.insert(k.to_string(), json!(v));
                        }
                        result_obj.insert("metadata".to_string(), Value::Object(meta_obj));
                    }
                    Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": Value::Object(result_obj),
                    }))
                }
                Err(error) => Ok(json_rpc_error(id, -32602, &error.to_string())),
            },
        };
    }

    if tool_name == "get_web_ui_url" {
        // Always allowed. When locked, point the caller at the browser-unlock
        // path so the password never needs to enter the conversation (§3.2).
        let gui_url = state.gui_url.read().await;
        let password_required = state.security.password_required();
        let unlocked = state.security.is_unlocked();
        let text = match gui_url.as_deref() {
            Some(url) => serde_json::to_string(&json!({
                "url": url,
                "mode": state.startup_mode,
                "available": true,
                "password_required": password_required,
                "unlocked": unlocked
            }))?,
            None => serde_json::to_string(&json!({
                "url": null,
                "mode": state.startup_mode,
                "available": false,
                "password_required": password_required,
                "unlocked": unlocked
            }))?,
        };

        return Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{
                    "type": "text",
                    "text": text
                }]
            }
        }));
    }

    Ok(json_rpc_error(
        id,
        -32601,
        &format!("Unknown tool: {tool_name}"),
    ))
}

/// Verify a password supplied via the `unlock_orb` tool and unlock the process.
/// On failure, applies the throttle delay (plan §2.6) before returning a
/// generic `-32002`, never leaking whether the Orb even has a password.
async fn handle_unlock_orb(state: &SharedState, id: Value, params: &Value) -> Value {
    // Already open (no password, or unlocked elsewhere in the process).
    if state.security.is_unlocked() {
        return unlock_success(id);
    }
    if !state.security.password_required() {
        return unlock_success(id);
    }

    let password = params
        .get("arguments")
        .and_then(|a| a.get("password"))
        .and_then(|p| p.as_str())
        .unwrap_or("");

    match crate::perform_unlock(state, password) {
        // Password-only Orbs verify here; encrypted Orbs decrypt + load before
        // unlocking (handled inside perform_unlock).
        Ok(()) => unlock_success(id),
        Err(_) => {
            let delay = state.security.backoff_delay();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            json_rpc_error(id, -32002, "Invalid password")
        }
    }
}

fn unlock_success(id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": "Orb unlocked." }] }
    })
}

fn handle_resource_read(state: &SharedState, id: Value, request: Value) -> Value {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let uri = params
        .get("uri")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let doc_id: Option<u32> = uri
        .strip_prefix("orb://documents/")
        .and_then(|value| value.parse().ok());
    let k = state.knowledge();
    if let Some(doc_id) = doc_id {
        if k.documents.iter().any(|document| document.id == doc_id) {
            let text: String = k
                .chunks
                .iter()
                .filter(|chunk| chunk.document_id == doc_id)
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            json!({ "jsonrpc": "2.0", "id": id, "result": { "contents": [{ "uri": uri, "mimeType": "text/plain", "text": text }] } })
        } else {
            json_rpc_error(id, -32602, "Document not found")
        }
    } else {
        json_rpc_error(id, -32602, "Invalid resource URI")
    }
}

fn json_rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

pub async fn post_streamable_http_mcp(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(request): Json<Value>,
) -> Response {
    let wants_sse = headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().starts_with("text/event-stream"))
        })
        .unwrap_or(false);

    match handle_json_rpc_request(&state, request).await {
        Ok(Some(response)) if wants_sse => match serde_json::to_string(&response) {
            Ok(body) => sse_response(body),
            Err(error) => internal_error_response(error.into()),
        },
        Ok(Some(response)) => Json(response).into_response(),
        Ok(None) => StatusCode::ACCEPTED.into_response(),
        Err(error) => internal_error_response(error),
    }
}

fn sse_response(data: String) -> Response {
    let mut response = format!("event: message\ndata: {data}\n\n").into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    response
}

fn internal_error_response(error: anyhow::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": error.to_string() })),
    )
        .into_response()
}

pub async fn run_stdio_loop(state: SharedState) -> anyhow::Result<()> {
    tracing::info!("MCP stdio loop started");

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
    tokio::task::spawn_blocking(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if tx.blocking_send(line).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    tracing::error!("stdin read error: {error}");
                    break;
                }
            }
        }
    });

    let stdout = std::io::stdout();

    while let Some(line) = rx.recv().await {
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("Invalid JSON-RPC: {error}");
                continue;
            }
        };

        let Some(response) = handle_json_rpc_request(&state, request).await? else {
            continue;
        };

        let response_str = serde_json::to_string(&response)?;
        let mut out = stdout.lock();
        writeln!(out, "{response_str}")?;
        out.flush()?;
    }

    Ok(())
}

/// Outcome of the auto-fill stage. Carries the (possibly downgraded) method,
/// the (possibly internally-generated) query vector, and structured metadata
/// to surface back through the MCP response.
pub struct PreparedRequest {
    pub method: SearchMethodRequest,
    pub query_vector: Option<Vec<f32>>,
    pub metadata: Vec<(&'static str, String)>,
}

/// Implements the downgrade matrix in spec §4.5. Called once per `search_knowledge`
/// invocation, before dispatch into `SearchRuntime::search()`.
///
/// Returns `Err(message)` only for the hard-fail case in §4.5 row 3:
/// the Orb manifest declares an `embedding_model_tar_sha256` that disagrees
/// with the runtime's compile-time SHA. Every other path returns `Ok(...)`
/// with metadata describing what happened.
#[cfg(feature = "vector-embedder")]
pub async fn auto_fill_query_vector(
    state: &SharedState,
    requested_method: SearchMethodRequest,
    query: &str,
    incoming_query_vector: Option<Vec<f32>>,
) -> Result<PreparedRequest, String> {
    let mut method = requested_method;
    let mut metadata: Vec<(&'static str, String)> = Vec::new();

    // If the caller supplied a vector, trust them. This is the original
    // pre-embedder contract; we don't second-guess it.
    if incoming_query_vector.is_some() {
        return Ok(PreparedRequest {
            method,
            query_vector: incoming_query_vector,
            metadata,
        });
    }

    let orb_has_dense = state
        .knowledge()
        .manifest
        .enabled_capabilities
        .iter()
        .any(|c| matches!(c, Capability::FlatVector | Capability::Hnsw));
    let needs_vector = (matches!(method, SearchMethodRequest::Auto) && orb_has_dense)
        || matches!(method, SearchMethodRequest::FlatVector)
        || (matches!(method, SearchMethodRequest::Hybrid) && orb_has_dense);

    if !needs_vector {
        return Ok(PreparedRequest {
            method,
            query_vector: None,
            metadata,
        });
    }

    // Snapshot the embedder slot. ArcSwap::load gives a Guard; we clone the
    // inner Arc cheaply so we don't hold the guard across the .await.
    let snapshot: Option<Arc<TractEmbedder>> = {
        let guard = state.embedder_slot.load();
        let inner: &Option<Arc<TractEmbedder>> = guard.as_ref();
        inner.clone()
    };

    let Some(embedder) = snapshot else {
        // Embedder not ready (still downloading / load failed). §4.5 rows 4 & 8.
        if matches!(method, SearchMethodRequest::FlatVector) {
            method = SearchMethodRequest::Auto;
            metadata.push(("degraded_from", "vector".to_string()));
            metadata.push(("reason", "embedder_not_ready".to_string()));
        }
        // For hybrid, dispatch will skip dense automatically — no method change.
        return Ok(PreparedRequest {
            method,
            query_vector: None,
            metadata,
        });
    };

    // SHA check per §4.5. Hard-reject only when the manifest declares a SHA
    // AND it disagrees with ours. Manifest with no SHA is legacy → fall through
    // to soft constraint (vector search itself will validate dimension).
    match state.knowledge().manifest.embedding_model_tar_sha256.as_deref() {
        Some(sha) if sha == mcporb_embed::MODEL_TAR_SHA256 => {
            // exact match — proceed
        }
        Some(_) => {
            return Err(format!(
                "embedding_model_mismatch: orb requires model {:?} (sha {}) but runtime has {} (sha {})",
                state.knowledge().manifest.embedding_model.as_deref().unwrap_or("<unknown>"),
                state.knowledge().manifest.embedding_model_tar_sha256.as_deref().unwrap_or("<unknown>"),
                mcporb_embed::MODEL_ID,
                mcporb_embed::MODEL_TAR_SHA256
            ));
        }
        None => {
            // Legacy orb — proceed under soft constraint
            metadata.push(("embedding_constraint", "soft".to_string()));
        }
    }

    match mcporb_embed::embed(embedder, query.to_string()).await {
        Ok(vec) => {
            metadata.push(("query_vector_source", "runtime_internal".to_string()));
            metadata.push(("embedding_model", mcporb_embed::MODEL_ID.to_string()));
            Ok(PreparedRequest {
                method,
                query_vector: Some(vec),
                metadata,
            })
        }
        Err(e) => Err(format!("embedder_failure: {}", e)),
    }
}

/// Lite-flavor stub: no embedder is compiled in, so this just passes the
/// caller's request through unchanged. If the Orb's manifest declares
/// `flat_vector` capability it should not have been packaged with the lite
/// runtime in the first place; `available_method_names()` will hide the
/// `vector` method from MCP `tools/list` anyway.
#[cfg(not(feature = "vector-embedder"))]
pub async fn auto_fill_query_vector(
    _state: &SharedState,
    requested_method: SearchMethodRequest,
    _query: &str,
    incoming_query_vector: Option<Vec<f32>>,
) -> Result<PreparedRequest, String> {
    Ok(PreparedRequest {
        method: requested_method,
        query_vector: incoming_query_vector,
        metadata: Vec::new(),
    })
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use crate::security::test_password_config;
    use crate::state::{LoadedKnowledge, OrbState};
    use mcporb_runtime_core::format::{Capability, RetrievalPlanKind};
    use mcporb_runtime_core::{build_bm25_index, Chunk, DenseRuntime, OrbManifest, SearchRuntime};

    /// A password-protected state with a single searchable chunk. `password` is
    /// "open-sesame-123"; the process starts locked.
    fn locked_state() -> SharedState {
        let chunks = vec![Chunk {
            id: 0,
            document_id: 0,
            section_id: None,
            page: Some(1),
            text: "alpha bravo charlie knowledge content".to_string(),
            token_count: 5,
        }];
        let manifest = OrbManifest {
            name: "secret-orb".to_string(),
            version: "0.1.0".to_string(),
            description: "guarded".to_string(),
            orb_format_version: "0.2".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            build_time: "2026-06-04T00:00:00Z".to_string(),
            source_documents: vec![],
            chunk_count: chunks.len(),
            index_format_version: "0.2".to_string(),
            binary_size_target_mb: 20,
            selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
            enabled_capabilities: vec![Capability::Bm25],
            embedding_dim: None,
            embedding_model: None,
            embedding_model_tar_sha256: None,
            trigram_min_df: None,
            planning_rationale: vec![],
        };
        let search = SearchRuntime {
            bm25: build_bm25_index(&chunks),
            tfidf: None,
            trigram: None,
            dense: DenseRuntime::None,
            dense_tier: RetrievalPlanKind::Bm25Only,
        };
        OrbState::new(
            test_password_config("open-sesame-123"),
            crate::state::LoadedAssets::Plain(LoadedKnowledge {
                manifest,
                documents: vec![],
                chunks,
                search,
            }),
            #[cfg(feature = "vector-embedder")]
            std::sync::Arc::new(mcporb_embed::ModelManager::with_cache_dir(
                tempfile::tempdir().unwrap().path().to_path_buf(),
            )),
            #[cfg(feature = "vector-embedder")]
            std::sync::Arc::new(mcporb_embed::empty_slot()),
            "AllGui".to_string(),
            None,
            Some("http://127.0.0.1:5599/tok/".to_string()),
        )
    }

    fn req(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    fn tool_names(list_result: &Value) -> Vec<String> {
        list_result["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[tokio::test]
    async fn locked_tools_list_only_exposes_unlock_affordances() {
        // Contract (§3.2): a locked Orb hides search_knowledge; shows only the
        // two unlock affordances.
        let state = locked_state();
        let resp = handle_json_rpc_request(&state, req("tools/list", json!({})))
            .await
            .unwrap()
            .unwrap();
        let names = tool_names(&resp);
        assert!(names.contains(&"unlock_orb".to_string()));
        assert!(names.contains(&"get_web_ui_url".to_string()));
        assert!(!names.contains(&"search_knowledge".to_string()));
    }

    #[tokio::test]
    async fn locked_search_knowledge_returns_auth_error() {
        // Contract (§4.4): calling a protected tool while locked is -32001.
        let state = locked_state();
        let resp = handle_json_rpc_request(
            &state,
            req(
                "tools/call",
                json!({ "name": "search_knowledge", "arguments": { "query": "alpha" } }),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(resp["error"]["code"], -32001);
        // Error guides the user to the browser-unlock path.
        assert!(resp["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Web UI"));
    }

    #[tokio::test]
    async fn wrong_then_right_password_unlocks_and_search_works() {
        // Contract: -32002 on wrong password; success + working search after the
        // right one; tools/list then includes search_knowledge.
        let state = locked_state();

        let bad = handle_json_rpc_request(
            &state,
            req(
                "tools/call",
                json!({ "name": "unlock_orb", "arguments": { "password": "nope" } }),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(bad["error"]["code"], -32002);
        assert!(!state.security.is_unlocked());

        let ok = handle_json_rpc_request(
            &state,
            req(
                "tools/call",
                json!({ "name": "unlock_orb", "arguments": { "password": "open-sesame-123" } }),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(ok["result"]["content"][0]["text"], "Orb unlocked.");
        assert!(state.security.is_unlocked());

        let names = tool_names(
            &handle_json_rpc_request(&state, req("tools/list", json!({})))
                .await
                .unwrap()
                .unwrap(),
        );
        assert!(names.contains(&"search_knowledge".to_string()));

        let search = handle_json_rpc_request(
            &state,
            req(
                "tools/call",
                json!({ "name": "search_knowledge", "arguments": { "query": "alpha" } }),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(search["result"]["content"].is_array());
    }

    #[tokio::test]
    async fn initialize_allowed_while_locked() {
        // Contract: initialize never gates (handshake must work pre-unlock).
        let state = locked_state();
        let resp = handle_json_rpc_request(&state, req("initialize", json!({})))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn get_web_ui_url_reports_lock_status_while_locked() {
        // Contract (§4.4): get_web_ui_url is allowed locked and advertises the
        // URL + lock status so the client can steer to browser unlock.
        let state = locked_state();
        let resp = handle_json_rpc_request(
            &state,
            req("tools/call", json!({ "name": "get_web_ui_url" })),
        )
        .await
        .unwrap()
        .unwrap();
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["password_required"], true);
        assert_eq!(parsed["unlocked"], false);
        assert_eq!(parsed["available"], true);
    }
}
