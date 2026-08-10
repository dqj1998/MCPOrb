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
    slug: &str,
    orb_zip: &Path,
    use_runner_wrapper: bool,
    registry_id: Option<&str>,
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
        json: build_stdio_json(runtime_binary, slug, orb_zip, use_runner_wrapper, registry_id),
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

/// Generate a single unified STDIO config pointing to the Gateway binary.
///
/// Unlike the per-orb `stdio_config_snippets`, this produces one entry —
/// `mcporb-gateway` — that routes to ALL installed Orbs via namespace prefix.
pub fn gateway_stdio_config_snippets(
    gateway_binary: &Path,
    registry_dir: &Path,
) -> Vec<McpConfigSnippet> {
    let value = json!({
        "mcpServers": {
            "mcporb-gateway": {
                "command": gateway_binary.display().to_string(),
                "args": [
                    "--registry-dir",
                    registry_dir.display().to_string(),
                ]
            }
        }
    });
    let json_str = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());

    // Same JSON for every client — the SDK reads the same format.
    [
        ("claude_desktop", "Claude Desktop"),
        ("cursor", "Cursor"),
        ("vscode", "VS Code"),
    ]
    .into_iter()
    .map(|(client, label)| McpConfigSnippet {
        client: client.to_string(),
        label: label.to_string(),
        json: json_str.clone(),
    })
    .collect()
}

/// Generate a single unified HTTP config pointing to the Gateway HTTP server.
/// When a token is configured it is embedded in the URL (`?token=`) so
/// URL-only MCP clients can authenticate without header support. `host` is
/// the connectable address (127.0.0.1 for localhost mode, LAN IP for
/// external mode).
pub fn gateway_http_config_snippets(
    port: u16,
    token: Option<&str>,
    host: &str,
) -> Vec<McpConfigSnippet> {
    let url = match token {
        Some(t) => format!("http://{host}:{port}/mcp?token={t}"),
        None => format!("http://{host}:{port}/mcp"),
    };
    let value = json!({
        "mcpServers": {
            "mcporb-gateway": {
                "url": url
            }
        }
    });
    let json_str = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());

    vec![McpConfigSnippet {
        client: "http".to_string(),
        label: "HTTP Gateway".to_string(),
        json: json_str,
    }]
}

fn build_stdio_json(
    runtime_binary: &Path,
    slug: &str,
    orb_zip: &Path,
    use_runner_wrapper: bool,
    registry_id: Option<&str>,
) -> String {
    let mut args: Vec<String> = if use_runner_wrapper {
        vec!["--mcp-stdio".into(), "--orb-zip".into(), orb_zip.display().to_string()]
    } else {
        vec!["--orb-zip".into(), orb_zip.display().to_string(), "--stdio-only".into()]
    };
    // Pass the registry hash so the runtime writes metrics where the Tauri app
    // reads them (registry root /metrics/<orb_id>.json).
    if let Some(id) = registry_id {
        args.push("--orb-id".into());
        args.push(id.to_string());
    }
    let value = json!({
        "mcpServers": {
            format!("mcporb-{slug}"): {
                "command": runtime_binary.display().to_string(),
                "args": args
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_direct_mode_uses_stdio_only_arg() {
        let bin = Path::new("/usr/local/bin/mcporb-runtime");
        let zip = Path::new("/tmp/test.orb.zip");
        let json = build_stdio_json(bin, "my-orb", zip, false, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let entry = &parsed["mcpServers"]["mcporb-my-orb"];
        assert_eq!(entry["command"], "/usr/local/bin/mcporb-runtime");
        assert_eq!(entry["args"][0], "--orb-zip");
        assert_eq!(entry["args"][1], "/tmp/test.orb.zip");
        assert_eq!(entry["args"][2], "--stdio-only");
    }

    #[test]
    fn stdio_wrapper_mode_uses_mcp_stdio_flag() {
        let bin = Path::new("/Applications/MCPOrb Runner.app/Contents/MacOS/mcporb-runner");
        let zip = Path::new("/tmp/test.orb.zip");
        let json = build_stdio_json(bin, "my-orb", zip, true, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let entry = &parsed["mcpServers"]["mcporb-my-orb"];
        assert_eq!(entry["command"], "/Applications/MCPOrb Runner.app/Contents/MacOS/mcporb-runner");
        assert_eq!(entry["args"][0], "--mcp-stdio");
        assert_eq!(entry["args"][1], "--orb-zip");
        assert_eq!(entry["args"][2], "/tmp/test.orb.zip");
        // --stdio-only must NOT appear in wrapper mode
        assert!(!entry["args"].as_array().unwrap().iter().any(|a| a == "--stdio-only"));
    }

    #[test]
    fn stdio_wrapper_mode_passes_orb_id() {
        let bin = Path::new("/usr/bin/mcporb-runner");
        let zip = Path::new("/tmp/test.orb.zip");
        let json = build_stdio_json(bin, "my-orb", zip, true, Some("a1b2c3d4e5f6g7h8"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let entry = &parsed["mcpServers"]["mcporb-my-orb"];
        assert_eq!(entry["args"][0], "--mcp-stdio");
        assert_eq!(entry["args"][1], "--orb-zip");
        assert_eq!(entry["args"][2], "/tmp/test.orb.zip");
        assert_eq!(entry["args"][3], "--orb-id");
        assert_eq!(entry["args"][4], "a1b2c3d4e5f6g7h8");
    }

    #[test]
    fn stdio_snippets_produces_all_clients() {
        let bin = Path::new("/usr/bin/mcporb-runtime");
        let zip = Path::new("/tmp/test.orb.zip");
        let snippets = stdio_config_snippets(bin, "test-orb", zip, false, None);
        assert_eq!(snippets.len(), 3);
        assert!(snippets.iter().any(|s| s.client == "claude_desktop"));
        assert!(snippets.iter().any(|s| s.client == "cursor"));
        assert!(snippets.iter().any(|s| s.client == "vscode"));
    }

    #[test]
    fn wrapper_snippet_args_match_direct_snippet_except_flag() {
        let bin = Path::new("/usr/bin/mcporb-runner");
        let zip = Path::new("/tmp/test.orb.zip");
        let direct = build_stdio_json(bin, "orb", zip, false, None);
        let wrapper = build_stdio_json(bin, "orb", zip, true, None);
        let d: serde_json::Value = serde_json::from_str(&direct).unwrap();
        let w: serde_json::Value = serde_json::from_str(&wrapper).unwrap();
        // command is the same binary path
        assert_eq!(d["mcpServers"]["mcporb-orb"]["command"], w["mcpServers"]["mcporb-orb"]["command"]);
        // direct: [--orb-zip, <path>, --stdio-only], 3 items
        let d_args = d["mcpServers"]["mcporb-orb"]["args"].as_array().unwrap();
        let w_args = w["mcpServers"]["mcporb-orb"]["args"].as_array().unwrap();
        assert_eq!(d_args.len(), 3);
        // wrapper has 3 base args + 0 registry_id (None passed)
        assert_eq!(w_args.len(), 3);
        // wrapper: [--mcp-stdio, --orb-zip, <path>]
        assert_eq!(w_args[0], "--mcp-stdio");
        assert_eq!(d_args[0], "--orb-zip");
        // both contain --orb-zip <path> at same position (shifted)
        assert_eq!(w_args[1], "--orb-zip");
        assert_eq!(w_args[2], "/tmp/test.orb.zip");
        // direct: [--orb-zip, <path>, --stdio-only]
        assert_eq!(d_args[1], "/tmp/test.orb.zip");
        assert_eq!(d_args[2], "--stdio-only");
    }

    #[test]
    fn gateway_http_snippets_embed_token_in_url() {
        let snippets = gateway_http_config_snippets(5599, Some("T0k3n-abc"), "127.0.0.1");
        assert_eq!(snippets.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&snippets[0].json).unwrap();
        let url = parsed["mcpServers"]["mcporb-gateway"]["url"].as_str().unwrap();
        assert_eq!(url, "http://127.0.0.1:5599/mcp?token=T0k3n-abc");
    }

    #[test]
    fn gateway_http_snippets_omit_token_when_none() {
        let snippets = gateway_http_config_snippets(5599, None, "127.0.0.1");
        let parsed: serde_json::Value = serde_json::from_str(&snippets[0].json).unwrap();
        let url = parsed["mcpServers"]["mcporb-gateway"]["url"].as_str().unwrap();
        assert_eq!(url, "http://127.0.0.1:5599/mcp");
        assert!(!url.contains('?'));
    }

    #[test]
    fn gateway_http_snippets_use_custom_host() {
        let snippets = gateway_http_config_snippets(5599, None, "192.168.1.5");
        let parsed: serde_json::Value = serde_json::from_str(&snippets[0].json).unwrap();
        let url = parsed["mcpServers"]["mcporb-gateway"]["url"].as_str().unwrap();
        assert_eq!(url, "http://192.168.1.5:5599/mcp");
    }

    #[test]
    fn gateway_http_snippets_custom_host_with_token_appends_query_token() {
        let snippets = gateway_http_config_snippets(5599, Some("T0k3n-abc"), "192.168.1.5");
        let parsed: serde_json::Value = serde_json::from_str(&snippets[0].json).unwrap();
        let url = parsed["mcpServers"]["mcporb-gateway"]["url"].as_str().unwrap();
        assert_eq!(url, "http://192.168.1.5:5599/mcp?token=T0k3n-abc");
    }
}
