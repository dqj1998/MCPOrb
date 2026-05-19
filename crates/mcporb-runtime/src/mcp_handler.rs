use std::io::{BufRead, Write};
use mcporb_runtime_core::bm25_search;
use serde_json::{json, Value};
use crate::state::SharedState;

pub async fn run_stdio_loop(state: SharedState) -> anyhow::Result<()> {
    tracing::info!("MCP stdio loop started");

    // Spawn blocking thread to read stdin lines, send over channel
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
    tokio::task::spawn_blocking(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if tx.blocking_send(l).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("stdin read error: {e}");
                    break;
                }
            }
        }
    });

    let stdout = std::io::stdout();

    while let Some(line) = rx.recv().await {
        if line.trim().is_empty() { continue; }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => { tracing::warn!("Invalid JSON-RPC: {e}"); continue; }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        {
            let mut metrics = state.metrics.write().await;
            metrics.mcp_request_count += 1;
        }

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {}, "resources": {} },
                    "serverInfo": { "name": state.manifest.name, "version": state.manifest.version }
                }
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "tools": [
                    {
                        "name": "search_knowledge",
                        "description": format!("Search the {} knowledge base", state.manifest.name),
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Search query" },
                                "top_k": { "type": "integer", "description": "Number of results (default: 5)" }
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
                ]}
            }),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if tool_name == "search_knowledge" {
                    let args = params.get("arguments").cloned().unwrap_or(json!({}));
                    let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
                    let top_k = args.get("top_k").and_then(|k| k.as_u64()).unwrap_or(5) as usize;
                    {
                        let mut metrics = state.metrics.write().await;
                        metrics.search_count += 1;
                    }
                    let results = bm25_search(&state.index, &query, top_k);
                    let content: Vec<Value> = results.iter().filter_map(|(chunk_id, score)| {
                        state.chunks.get(*chunk_id as usize).map(|chunk| {
                            let preview = &chunk.text[..chunk.text.len().min(500)];
                            json!({
                                "type": "text",
                                "text": format!("[BM25 Score: {score:.3}] Page {:?}\n{preview}", chunk.page)
                            })
                        })
                    }).collect();
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "content": content } })
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
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("Unknown tool: {tool_name}") } })
                }
            },
            "resources/list" => {
                let resources: Vec<Value> = state.documents.iter().map(|d| json!({
                    "uri": format!("orb://documents/{}", d.id),
                    "name": d.title,
                    "mimeType": "text/plain"
                })).collect();
                json!({ "jsonrpc": "2.0", "id": id, "result": { "resources": resources } })
            },
            "resources/read" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("").to_string();
                let doc_id: Option<u32> = uri.strip_prefix("orb://documents/").and_then(|s| s.parse().ok());
                if let Some(did) = doc_id {
                    if state.documents.iter().any(|d| d.id == did) {
                        let text: String = state.chunks.iter()
                            .filter(|c| c.document_id == did)
                            .map(|c| c.text.as_str())
                            .collect::<Vec<_>>().join("\n\n");
                        json!({ "jsonrpc": "2.0", "id": id, "result": { "contents": [{ "uri": uri, "mimeType": "text/plain", "text": text }] } })
                    } else {
                        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": "Document not found" } })
                    }
                } else {
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": "Invalid resource URI" } })
                }
            },
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
