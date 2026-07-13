//! MCP protocol handler for the gateway.
//!
//! Handles JSON-RPC requests by either responding directly (initialize,
//! tools/list, notifications) or routing to the appropriate Orb child
//! process (tools/call, resources/read).

use serde_json::{json, Value};
use tracing;

use crate::router::{extract_slug_from_resource_uri, parse_tool_name};
use crate::runtime_manager::RuntimeManager;

/// Error codes matching MCP spec conventions.
pub mod error_codes {
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const ORB_NOT_FOUND: i64 = -32000;
    pub const ORB_UNAVAILABLE: i64 = -32001;
}

/// Build a standard JSON-RPC error response.
fn json_rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

/// Build a standard JSON-RPC success response.
fn json_rpc_success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

/// Handle a single (non-batch) JSON-RPC request.
///
/// All errors are returned as JSON-RPC error responses, never as `Err`.
/// The `Err` variant of `handle_tool_call`/`handle_resource_read` is an
/// internal error (e.g. process spawn failure) and is caught here, converted
/// to a generic server error response.
async fn handle_single_request(
    manager: &RuntimeManager,
    request: &Value,
) -> Result<Option<Value>, String> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Notifications — no response expected
    if matches!(method, "notifications/initialized" | "$/cancelRequest") {
        return Ok(None);
    }

    let response = match method {
        "initialize" => Some(handle_initialize(id, request)),
        "tools/list" => Some(handle_list_tools(manager, id)),
        "tools/call" => match handle_tool_call(manager, id.clone(), request).await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(%e, "Internal error handling tools/call");
                Some(json_rpc_error(id, error_codes::ORB_UNAVAILABLE, &format!("Internal error: {e}")))
            }
        },
        "resources/list" => Some(handle_list_resources(manager, id)),
        "resources/read" => match handle_resource_read(manager, id.clone(), request).await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(%e, "Internal error handling resources/read");
                Some(json_rpc_error(id, error_codes::ORB_UNAVAILABLE, &format!("Internal error: {e}")))
            }
        },
        "ping" => Some(json_rpc_success(id, json!({}))),
        _ => Some(json_rpc_error(
            id,
            error_codes::METHOD_NOT_FOUND,
            &format!("Method not found: {method}"),
        )),
    };

    Ok(response)
}

/// Handle the `initialize` handshake.
fn handle_initialize(id: Value, _request: &Value) -> Value {
    json_rpc_success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "MCPOrb Gateway",
                "version": "0.1.0",
                "description": "Unified MCP gateway for multiple knowledge Orbs — routes requests to the appropriate Orb by namespace prefix."
            }
        }),
    )
}

/// Handle `tools/list` — aggregate tools from all known Orbs.
fn handle_list_tools(manager: &RuntimeManager, id: Value) -> Value {
    let orbs = manager.list_orbs();

    let tools: Vec<Value> = orbs
        .iter()
        .flat_map(|orb| {
            orb.tools.iter().map(|tool| {
                json!({
                    "name": tool.namespaced_name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema,
                })
            })
        })
        .collect();

    json_rpc_success(id, json!({ "tools": tools }))
}

/// Handle `tools/call` — parse namespace, route to Orb.
async fn handle_tool_call(
    manager: &RuntimeManager,
    id: Value,
    request: &Value,
) -> Result<Option<Value>, String> {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Parse the namespaced tool name
    let (slug, original_method) = parse_tool_name(tool_name).ok_or_else(|| {
        format!(
            "Invalid tool name format: '{tool_name}'. Expected '{{orb}}__search_knowledge'."
        )
    })?;

    if original_method != "search_knowledge" {
        return Ok(Some(json_rpc_error(
            id,
            error_codes::METHOD_NOT_FOUND,
            &format!("Unknown tool method: {original_method}"),
        )));
    }

    // Check the orb exists
    manager.find_orb(slug).ok_or_else(|| {
        format!("Unknown Orb: '{slug}'. Available Orbs: {}", {
            let names: Vec<&str> = manager.list_orbs().iter().map(|o| o.slug.as_str()).collect();
            names.join(", ")
        })
    })?;

    // Extract the arguments (strip the outer params wrapper)
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    // Forward to the Orb
    let forward_params = json!({
        "name": original_method,
        "arguments": arguments,
    });

    match manager
        .forward_request(slug, "tools/call", forward_params)
        .await
    {
        Ok(response) => {
            // The response is a full JSON-RPC response from the child.
            // Extract the result part and wrap with our id.
            if let Some(result) = response.get("result") {
                Ok(Some(json_rpc_success(id, result.clone())))
            } else if let Some(error) = response.get("error") {
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": error.clone()
                })))
            } else {
                // Pass through the entire response
                Ok(Some(response))
            }
        }
        Err(e) => {
            let msg = format!("Orb '{slug}' error: {e}");
            tracing::error!(%msg);
            Ok(Some(json_rpc_error(
                id,
                error_codes::ORB_UNAVAILABLE,
                &msg,
            )))
        }
    }
}

/// Handle `resources/list` — aggregate resources from all known Orbs.
///
/// For v1, returns an empty list (resources require reading each orb's
/// documents.postcard from the zip, which is deferred to a future version).
fn handle_list_resources(manager: &RuntimeManager, id: Value) -> Value {
    // v1: collect resources from the orb manifests
    let orbs = manager.list_orbs();

    let mut resources = Vec::new();
    for orb in orbs {
        // We can't get document-level info without reading the zip,
        // so for v1 we just return an informational resource.
        resources.push(json!({
            "uri": format!("orb://{}/", orb.slug),
            "name": format!("{} Knowledge Base", orb.display_name),
            "description": format!("Access the full knowledge base of {}", orb.display_name),
            "mimeType": "text/plain"
        }));
    }

    json_rpc_success(id, json!({ "resources": resources }))
}

/// Handle `resources/read` — parse namespace URI, route to Orb.
async fn handle_resource_read(
    manager: &RuntimeManager,
    id: Value,
    request: &Value,
) -> Result<Option<Value>, String> {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Extract slug from resource URI
    let slug = extract_slug_from_resource_uri(uri).ok_or_else(|| {
        format!(
            "Invalid resource URI format: '{uri}'. Expected 'orb://{{slug}}/...'."
        )
    })?;

    // Check the orb exists
    if !manager.has_orb(slug) {
        return Ok(Some(json_rpc_error(
            id,
            error_codes::ORB_NOT_FOUND,
            &format!("Unknown Orb: '{slug}'"),
        )));
    }

    // Forward the resources/read request as-is (URI stays in orb-native format)
    match manager
        .forward_request(slug, "resources/read", params.clone())
        .await
    {
        Ok(response) => {
            if let Some(result) = response.get("result") {
                Ok(Some(json_rpc_success(id, result.clone())))
            } else if let Some(error) = response.get("error") {
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": error.clone()
                })))
            } else {
                Ok(Some(response))
            }
        }
        Err(e) => {
            let msg = format!("Orb '{slug}' error: {e}");
            Ok(Some(json_rpc_error(
                id,
                error_codes::ORB_UNAVAILABLE,
                &msg,
            )))
        }
    }
}

/// Handle a JSON-RPC request (supports single requests and batches).
///
/// Returns `Ok(Some(response))` for requests that expect a response,
/// `Ok(None)` for notifications, and `Err` for parse errors.
pub async fn handle_request(
    manager: &RuntimeManager,
    request: Value,
) -> Result<Option<Value>, String> {
    // Batch request
    if let Some(batch) = request.as_array() {
        if batch.is_empty() {
            return Ok(Some(json_rpc_error(
                Value::Null,
                error_codes::INVALID_REQUEST,
                "Empty batch array",
            )));
        }

        let mut responses = Vec::new();
        for item in batch {
            if let Some(response) = handle_single_request(manager, item).await? {
                responses.push(response);
            }
        }

        return if responses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Value::Array(responses)))
        };
    }

    // Single request
    if !request.is_object() {
        return Ok(Some(json_rpc_error(
            Value::Null,
            error_codes::INVALID_REQUEST,
            "Invalid Request: expected JSON object",
        )));
    }

    handle_single_request(manager, &request).await
}

/// Build a human-readable tool list summary for error messages.
pub fn format_available_tools(manager: &RuntimeManager) -> String {
    let orbs = manager.list_orbs();
    let tool_names: Vec<String> = orbs
        .iter()
        .flat_map(|orb| {
            orb.tools
                .iter()
                .map(|t| t.namespaced_name.clone())
        })
        .collect();
    tool_names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GatewayTool;
    use crate::registry_reader::GatewayConfig;
    use crate::runtime_manager::RuntimeManager;

    fn make_gateway_tool(slug: &str) -> GatewayTool {
        GatewayTool {
            original_name: "search_knowledge".to_string(),
            namespaced_name: crate::router::build_namespaced_tool_name(slug, "search_knowledge"),
            description: format!("Search the {slug} knowledge base"),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        }
    }

    fn test_manager() -> RuntimeManager {
        let config = GatewayConfig::default();
        let orbs = vec![
            crate::registry_reader::GatewayOrb {
                id: "id1".to_string(),
                slug: "orb-a".to_string(),
                display_name: "Orb A".to_string(),
                description: "First test orb".to_string(),
                zip_path: std::path::PathBuf::from("/tmp/orb-a.zip"),
                mcp_protocol_version: "2024-11-05".to_string(),
                tools: vec![make_gateway_tool("orb-a")],
            },
            crate::registry_reader::GatewayOrb {
                id: "id2".to_string(),
                slug: "orb-b".to_string(),
                display_name: "Orb B".to_string(),
                description: "Second test orb".to_string(),
                zip_path: std::path::PathBuf::from("/tmp/orb-b.zip"),
                mcp_protocol_version: "2024-11-05".to_string(),
                tools: vec![make_gateway_tool("orb-b")],
            },
        ];
        RuntimeManager::new(config, orbs)
    }

    #[tokio::test]
    async fn initialize_returns_gateway_info() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1.0" }
            }
        });

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(response["result"]["serverInfo"]["name"], "MCPOrb Gateway");
    }

    #[tokio::test]
    async fn tools_list_aggregates_all_orbs() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);

        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"orb-a__search_knowledge"));
        assert!(names.contains(&"orb-b__search_knowledge"));
    }

    #[tokio::test]
    async fn tool_call_to_nonexistent_orb_returns_error() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "nonexistent__search_knowledge",
                "arguments": { "query": "test" }
            }
        });

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        assert!(response.get("error").is_some(), "expected error response, got: {response}");
        // The error code is ORB_UNAVAILABLE because the error bubbles up through
        // the outer match that catches internal errors from handle_tool_call.
        assert!(
            response["error"]["code"] == error_codes::ORB_UNAVAILABLE
                || response["error"]["code"] == error_codes::ORB_NOT_FOUND,
            "unexpected error code: {} ({})",
            response["error"]["code"],
            response["error"]["message"]
        );
    }

    #[tokio::test]
    async fn tool_call_missing_namespace_returns_error() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "search_knowledge",
                "arguments": { "query": "test" }
            }
        });

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn notification_returns_none() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let response = handle_request(&manager, request).await.unwrap();
        assert!(response.is_none());
    }

    #[tokio::test]
    async fn cancel_request_returns_none() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "$/cancelRequest",
            "params": { "id": 1 }
        });

        let response = handle_request(&manager, request).await.unwrap();
        assert!(response.is_none());
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "unknown_method"
        });

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        assert_eq!(response["error"]["code"], error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn resources_list_returns_entries() {
        let manager = test_manager();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list"
        });

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        let resources = response["result"]["resources"].as_array().unwrap();
        assert_eq!(resources.len(), 2);

        let uris: Vec<&str> = resources
            .iter()
            .filter_map(|r| r["uri"].as_str())
            .collect();
        assert!(uris.contains(&"orb://orb-a/"));
        assert!(uris.contains(&"orb://orb-b/"));
    }

    #[tokio::test]
    async fn empty_batch_returns_error() {
        let manager = test_manager();
        let request = json!([]);

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn batch_request() {
        let manager = test_manager();
        let request = json!([
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            },
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list"
            }
        ]);

        let response = handle_request(&manager, request).await.unwrap().unwrap();
        let responses = response.as_array().unwrap();
        assert_eq!(responses.len(), 2);
        assert!(responses[0].get("result").is_some());
    }
}
