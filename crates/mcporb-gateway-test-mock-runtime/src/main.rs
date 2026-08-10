//! Mock `mcporb-runtime` binary for gateway integration tests.
//!
//! Mimics the real `mcporb-runtime --stdio-only` behavior:
//!   - Reads JSON-RPC lines from stdin
//!   - Responds to `initialize` with a proper initialized response
//!   - Responds to `tools/call` with a simple search result
//!   - Responds to `tools/list` with a search_knowledge tool
//!   - Writes JSON-RPC response lines to stdout

use std::io::{BufRead, Write};

fn handle_initialize(id: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {}, "resources": {} },
            "serverInfo": {
                "name": "Mock Runtime",
                "version": "0.1.0"
            }
        }
    })
}

fn handle_ping(id: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {}
    })
}

fn handle_tools_list(id: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "search_knowledge",
                    "description": "Search the knowledge base",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" },
                            "top_k": { "type": "integer", "default": 5 }
                        },
                        "required": ["query"]
                    }
                }
            ]
        }
    })
}

fn handle_tools_call(id: &serde_json::Value, params: &serde_json::Value) -> serde_json::Value {
    let arguments = params.get("arguments").cloned().unwrap_or_default();
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [
                {
                    "type": "text",
                    "text": format!("Mock result for query: {query}")
                }
            ],
            "isError": false
        }
    })
}

fn handle_resources_list(id: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "resources": []
        }
    })
}

// Mirrors the real orb runtime (crates/mcporb-runtime/src/mcp_handler.rs):
// only `orb://documents/{id}` is accepted; the response echoes the native URI.
fn handle_resources_read(
    id: &serde_json::Value,
    params: &serde_json::Value,
) -> serde_json::Value {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let doc_id: Option<u32> = uri
        .strip_prefix("orb://documents/")
        .and_then(|value| value.parse().ok());
    match doc_id {
        Some(doc_id) => serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "contents": [{
                    "uri": uri,
                    "mimeType": "text/plain",
                    "text": format!("Mock document {doc_id}")
                }]
            }
        }),
        None => serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "Invalid resource URI" }
        }),
    }
}

fn handle_request(request: &serde_json::Value) -> Option<serde_json::Value> {
    let id = request.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let method = request
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Notifications — no response
    if matches!(method, "notifications/initialized") {
        return None;
    }

    let params = request.get("params").cloned().unwrap_or_default();

    let response = match method {
        "initialize" => handle_initialize(&id),
        "ping" => handle_ping(&id),
        "tools/list" => handle_tools_list(&id),
        "tools/call" => handle_tools_call(&id, &params),
        "resources/list" => handle_resources_list(&id),
        "resources/read" => handle_resources_read(&id, &params),
        _ => serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    };

    Some(response)
}

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                let error = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                let mut out = stdout.lock();
                let _ = writeln!(out, "{}", serde_json::to_string(&error).unwrap());
                continue;
            }
        };

        if let Some(response) = handle_request(&request) {
            let mut out = stdout.lock();
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = writeln!(out, "{json}");
                let _ = out.flush();
            }
        }
    }
}
