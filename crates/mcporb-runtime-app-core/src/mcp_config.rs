use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfigSnippet {
    pub client: String,
    pub label: String,
    pub json: String,
}

pub fn stdio_config_snippets(
    runtime_binary: &Path,
    orb_id: &str,
    orb_zip: &Path,
) -> Vec<McpConfigSnippet> {
    [
        ("claude_desktop", "Claude Desktop"),
        ("cursor", "Cursor"),
        ("vscode", "VS Code"),
    ]
    .into_iter()
    .map(|(client, label)| McpConfigSnippet {
        client: client.to_string(),
        label: label.to_string(),
        json: build_stdio_json(runtime_binary, orb_id, orb_zip),
    })
    .collect()
}

pub fn http_config_snippets(orb_id: &str, port: u16, token: &str) -> Vec<McpConfigSnippet> {
    let url = format!("http://127.0.0.1:{port}/{token}/mcp");
    [
        ("claude_desktop", "Claude Desktop (HTTP)"),
        ("cursor", "Cursor (HTTP)"),
        ("vscode", "VS Code (HTTP)"),
    ]
    .into_iter()
    .map(|(client, label)| McpConfigSnippet {
        client: client.to_string(),
        label: label.to_string(),
        json: build_http_json(orb_id, &url),
    })
    .collect()
}

fn build_stdio_json(runtime_binary: &Path, orb_id: &str, orb_zip: &Path) -> String {
    let value = json!({
        "mcpServers": {
            format!("mcporb-{orb_id}"): {
                "command": runtime_binary.display().to_string(),
                "args": ["--orb-zip", orb_zip.display().to_string(), "--stdio-only"]
            }
        }
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

fn build_http_json(orb_id: &str, url: &str) -> String {
    let value = json!({
        "mcpServers": {
            format!("mcporb-{orb_id}"): {
                "url": url
            }
        }
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}
