use std::io::{BufRead, Write};

use mcporb_runtime_core::{SearchMethodRequest, SearchRequest};
use serde_json::{json, Value};

use crate::state::SharedState;

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
                    "serverInfo": { "name": state.manifest.name, "version": state.manifest.version }
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
                                        "description": "Search method override",
                                        "enum": state.search.available_method_names()
                                    },
                                    "query_vector": {
                                        "type": "array",
                                        "description": "Dense query vector required by method=vector and dense hybrid",
                                        "items": { "type": "number" }
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
                        match state.search.search(&SearchRequest {
                            query,
                            top_k,
                            method: SearchMethodRequest::from_str(method_name),
                            query_vector,
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
                                json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "content": content,
                                        "active_plan": result.active_plan.to_string()
                                    }
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
