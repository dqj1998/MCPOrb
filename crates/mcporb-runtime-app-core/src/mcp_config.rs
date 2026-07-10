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
    use_runner_wrapper: bool,
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
        json: build_stdio_json(runtime_binary, orb_id, orb_zip, use_runner_wrapper),
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

fn build_stdio_json(runtime_binary: &Path, orb_id: &str, orb_zip: &Path, use_runner_wrapper: bool) -> String {
    let args: Vec<String> = if use_runner_wrapper {
        vec!["--mcp-stdio".into(), "--orb-zip".into(), orb_zip.display().to_string()]
    } else {
        vec!["--orb-zip".into(), orb_zip.display().to_string(), "--stdio-only".into()]
    };
    let value = json!({
        "mcpServers": {
            format!("mcporb-{orb_id}"): {
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
        let json = build_stdio_json(bin, "my-orb", zip, false);
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
        let json = build_stdio_json(bin, "my-orb", zip, true);
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
    fn stdio_snippets_produces_all_clients() {
        let bin = Path::new("/usr/bin/mcporb-runtime");
        let zip = Path::new("/tmp/test.orb.zip");
        let snippets = stdio_config_snippets(bin, "test-orb", zip, false);
        assert_eq!(snippets.len(), 3);
        assert!(snippets.iter().any(|s| s.client == "claude_desktop"));
        assert!(snippets.iter().any(|s| s.client == "cursor"));
        assert!(snippets.iter().any(|s| s.client == "vscode"));
    }

    #[test]
    fn wrapper_snippet_args_match_direct_snippet_except_flag() {
        let bin = Path::new("/usr/bin/mcporb-runner");
        let zip = Path::new("/tmp/test.orb.zip");
        let direct = build_stdio_json(bin, "orb", zip, false);
        let wrapper = build_stdio_json(bin, "orb", zip, true);
        let d: serde_json::Value = serde_json::from_str(&direct).unwrap();
        let w: serde_json::Value = serde_json::from_str(&wrapper).unwrap();
        // command is the same binary path
        assert_eq!(d["mcpServers"]["mcporb-orb"]["command"], w["mcpServers"]["mcporb-orb"]["command"]);
        // args differ only by the mode flag
        let d_args = d["mcpServers"]["mcporb-orb"]["args"].as_array().unwrap();
        let w_args = w["mcpServers"]["mcporb-orb"]["args"].as_array().unwrap();
        assert_eq!(d_args.len(), 3);
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
}
