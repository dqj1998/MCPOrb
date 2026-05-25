use std::io::{BufRead, Write};
#[cfg(feature = "vector-embedder")]
use std::sync::Arc;

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
    let mut parts: Vec<&str> = vec![
        "Search method (default: auto).",
    ];
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
        parts.push("'vector': semantic similarity search, best for conceptual or paraphrase queries.");
    }
    if methods.contains(&"hybrid") {
        parts.push("'hybrid': fuses all available rankers via RRF, recommended for mixed queries.");
    }
    parts.join(" ")
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

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|value| value.as_str()).unwrap_or("");

        {
            let mut metrics = state.metrics.write().await;
            metrics.mcp_request_count += 1;
        }

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {}, "resources": {} },
                    "serverInfo": {
                        "name": state.manifest.name,
                        "version": state.manifest.version,
                        "description": state.manifest.description
                    }
                }
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "search_knowledge",
                            "description": format!("Search the {} knowledge base", state.manifest.name),
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": { "type": "string", "description": "Search query" },
                                    "top_k": { "type": "integer", "description": "Number of results (default: 5)" },
                                    "method": {
                                        "type": "string",
                                        "description": build_method_description(&state.search.available_method_names()),
                                        "enum": state.search.available_method_names()
                                    }
                                },
                                "required": ["query"]
                            }
                        },
                        {
                            "name": "get_web_ui_url",
                            "description": "Get the local Web UI URL for this Orb when GUI mode is enabled",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        }
                    ]
                }
            }),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                let tool_name = params.get("name").and_then(|value| value.as_str()).unwrap_or("");

                if tool_name == "search_knowledge" {
                    let args = params.get("arguments").cloned().unwrap_or(json!({}));
                    let query = args.get("query").and_then(|value| value.as_str()).unwrap_or("").to_string();
                    let top_k = args.get("top_k").and_then(|value| value.as_u64()).unwrap_or(5) as usize;
                    let method_name = args.get("method").and_then(|value| value.as_str()).unwrap_or("auto");
                    let query_vector = args.get("query_vector").and_then(|value| value.as_array()).map(|values| {
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

                    let available_methods = state.search.available_method_names();
                    if !available_methods.iter().any(|value| *value == method_name) {
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32602,
                                "message": format!("Unsupported method: {method_name}. Available methods: {}", available_methods.join(", "))
                            }
                        })
                    } else {
                        let requested_method = SearchMethodRequest::from_str(method_name);
                        match auto_fill_query_vector(&state, requested_method, &query, query_vector).await {
                            Err(msg) => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32602,
                                    "message": msg,
                                }
                            }),
                            Ok(prepared) => match state.search.search(&SearchRequest {
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
                                            state.chunks.get(hit.chunk_id as usize).map(|chunk| {
                                                let preview = &chunk.text[..chunk.text.len().min(500)];
                                                json!({
                                                    "type": "text",
                                                    "text": format!("[{} Score: {:.3}] Page {:?}\n{}", hit.method, hit.score, chunk.page, preview)
                                                })
                                            })
                                        })
                                        .collect();
                                    let mut result_obj = serde_json::Map::new();
                                    result_obj.insert("content".to_string(), json!(content));
                                    result_obj.insert("active_plan".to_string(), json!(result.active_plan.to_string()));
                                    if !prepared.metadata.is_empty() {
                                        let mut meta_obj = serde_json::Map::new();
                                        for (k, v) in &prepared.metadata {
                                            meta_obj.insert(k.to_string(), json!(v));
                                        }
                                        result_obj.insert("metadata".to_string(), Value::Object(meta_obj));
                                    }
                                    json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": Value::Object(result_obj),
                                    })
                                }
                                Err(error) => json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "error": {
                                        "code": -32602,
                                        "message": error.to_string()
                                    }
                                }),
                            },
                        }
                    }
                } else if tool_name == "get_web_ui_url" {
                    let gui_url = state.gui_url.read().await;
                    let text = match gui_url.as_deref() {
                        Some(url) => serde_json::to_string(&json!({
                            "url": url,
                            "mode": state.startup_mode,
                            "available": true
                        }))?,
                        None => serde_json::to_string(&json!({
                            "url": null,
                            "mode": state.startup_mode,
                            "available": false
                        }))?,
                    };

                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": text
                            }]
                        }
                    })
                } else {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": format!("Unknown tool: {tool_name}")
                        }
                    })
                }
            }
            "resources/list" => {
                let resources: Vec<Value> = state.documents.iter().map(|document| json!({
                    "uri": format!("orb://documents/{}", document.id),
                    "name": document.title,
                    "mimeType": "text/plain"
                })).collect();
                json!({ "jsonrpc": "2.0", "id": id, "result": { "resources": resources } })
            }
            "resources/read" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                let uri = params.get("uri").and_then(|value| value.as_str()).unwrap_or("").to_string();
                let doc_id: Option<u32> = uri.strip_prefix("orb://documents/").and_then(|value| value.parse().ok());
                if let Some(doc_id) = doc_id {
                    if state.documents.iter().any(|document| document.id == doc_id) {
                        let text: String = state
                            .chunks
                            .iter()
                            .filter(|chunk| chunk.document_id == doc_id)
                            .map(|chunk| chunk.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        json!({ "jsonrpc": "2.0", "id": id, "result": { "contents": [{ "uri": uri, "mimeType": "text/plain", "text": text }] } })
                    } else {
                        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": "Document not found" } })
                    }
                } else {
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": "Invalid resource URI" } })
                }
            }
            "notifications/initialized" | "$/cancelRequest" => continue,
            _ => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("Method not found: {method}") } }),
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
        .manifest
        .enabled_capabilities
        .iter()
        .any(|c| matches!(c, Capability::FlatVector | Capability::Hnsw));
    let needs_vector = matches!(method, SearchMethodRequest::FlatVector)
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
    match state.manifest.embedding_model_tar_sha256.as_deref() {
        Some(sha) if sha == mcporb_embed::MODEL_TAR_SHA256 => {
            // exact match — proceed
        }
        Some(_) => {
            return Err(format!(
                "embedding_model_mismatch: orb requires model {:?} (sha {}) but runtime has {} (sha {})",
                state.manifest.embedding_model.as_deref().unwrap_or("<unknown>"),
                state.manifest.embedding_model_tar_sha256.as_deref().unwrap_or("<unknown>"),
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
