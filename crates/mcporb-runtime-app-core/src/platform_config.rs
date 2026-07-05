//! Discover, read, and write LLM platform MCP configuration files.
//!
//! Supported platforms:
//! - Claude Desktop (`claude_desktop_config.json`)
//! - Cursor (`mcp.json`)
//! - Cline (VS Code ext, `cline_mcp_settings.json`)
//! - Roo Code (VS Code ext fork, `mcp_settings.json`)
//! - Windsurf / Codeium (`mcp_config.json`)
//! - Zed Editor (`settings.json`, `context_servers` format)
//! - Continue.dev (`config.json`, array format)
//!
//! Each platform's config file is discovered at a well-known OS-specific path,
//! read back as raw JSON text, and can be overwritten (with automatic backup).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Data types ──────────────────────────────────────────────────────────────

/// A discovered LLM platform config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    /// Machine-readable platform identifier (e.g. `"claude_desktop"`, `"cursor"`).
    pub platform: String,
    /// Human-readable display name (e.g. `"Claude Desktop"`).
    pub display_name: String,
    /// Absolute path to the config file on disk.
    pub config_path: String,
    /// Whether the file currently exists on disk.
    pub exists: bool,
    /// Raw JSON content of the file (if exists).
    pub current_content: Option<String>,
    /// Error message if reading failed.
    pub read_error: Option<String>,
    /// The MCPOrb-generated config snippet that should be written.
    pub generated_content: Option<String>,
    /// Post-write hint (e.g. "Restart Claude Desktop").
    pub restart_hint: Option<String>,
    /// The user-readable config location label.
    pub location_label: String,
}

/// Result of writing a platform config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteConfigResult {
    /// Which platform was written.
    pub platform: String,
    /// Whether the write succeeded.
    pub success: bool,
    /// Path to the config file that was written.
    pub config_path: String,
    /// Path to the backup file (if created).
    pub backup_path: Option<String>,
    /// Error message if write failed.
    pub error: Option<String>,
    /// Post-write hint for the user.
    pub restart_hint: Option<String>,
}

// ── Platform definitions ────────────────────────────────────────────────────

struct PlatformInfo {
    platform: &'static str,
    display_name: &'static str,
    restart_hint_i18n_key: &'static str,
    /// Relative path from the platform's config directory.
    relative_path: &'static str,
}

/// Known platforms in display order.
const PLATFORMS: &[PlatformInfo] = &[
    PlatformInfo {
        platform: "claude_desktop",
        display_name: "Claude Desktop",
        restart_hint_i18n_key: "mcp.restart_hint.claude_desktop",
        relative_path: "claude_desktop_config.json",
    },
    PlatformInfo {
        platform: "cursor",
        display_name: "Cursor",
        restart_hint_i18n_key: "mcp.restart_hint.cursor",
        relative_path: "mcp.json",
    },
    PlatformInfo {
        platform: "cline",
        display_name: "Cline (VS Code)",
        restart_hint_i18n_key: "mcp.restart_hint.cline",
        relative_path: "globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
    },
    PlatformInfo {
        platform: "roo_code",
        display_name: "Roo Code (VS Code)",
        restart_hint_i18n_key: "mcp.restart_hint.roo_code",
        relative_path: "globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json",
    },
    PlatformInfo {
        platform: "windsurf",
        display_name: "Windsurf",
        restart_hint_i18n_key: "mcp.restart_hint.windsurf",
        relative_path: "mcp_config.json",
    },
    PlatformInfo {
        platform: "zed",
        display_name: "Zed Editor",
        restart_hint_i18n_key: "mcp.restart_hint.zed",
        relative_path: "settings.json",
    },
    PlatformInfo {
        platform: "continue_dev",
        display_name: "Continue.dev",
        restart_hint_i18n_key: "mcp.restart_hint.continue_dev",
        relative_path: "config.json",
    },
];

// ── Config directory resolution ─────────────────────────────────────────────

/// Resolve the base config directory for each platform on the current OS.
fn platform_config_dir(platform: &str) -> Option<PathBuf> {
    // dirs::config_dir() normally resolves to:
    //   macOS:   ~/Library/Application Support
    //   Windows: %APPDATA%
    //   Linux:   ~/.config
    let config_dir = dirs::config_dir()?;
    let data_dir = dirs::data_dir()?;

    match platform {
        "claude_desktop" => {
            // macOS:   ~/Library/Application Support/Claude
            // Windows: %APPDATA%/Claude
            // Linux:   ~/.config/Claude  (or ~/.local/share/Claude)
            if cfg!(target_os = "macos") {
                Some(config_dir.join("Claude"))
            } else if cfg!(target_os = "windows") {
                Some(config_dir.join("Claude"))
            } else {
                // Linux: try ~/.config/Claude first, then ~/.local/share/Claude
                let p1 = config_dir.join("Claude");
                if p1.is_dir() { Some(p1) } else { Some(data_dir.join("Claude")) }
            }
        }
        "cursor" => {
            // Cursor stores mcp.json at ~/.cursor/mcp.json (cross-platform)
            Some(dirs::home_dir()?.join(".cursor"))
        }
        "cline" | "roo_code" => {
            // VS Code globalStorage: varies by OS
            // macOS:   ~/Library/Application Support/Code/User/globalStorage
            // Windows: %APPDATA%/Code/User/globalStorage
            // Linux:   ~/.config/Code/User/globalStorage
            let vscode_config = if cfg!(target_os = "macos") {
                config_dir.join("Code")
            } else if cfg!(target_os = "windows") {
                config_dir.join("Code")
            } else {
                config_dir.join("Code")
            };
            Some(vscode_config.join("User"))
        }
        "windsurf" => {
            // Windsurf/Codeium stores config at ~/.codeium/windsurf/ (cross-platform)
            Some(dirs::home_dir()?.join(".codeium").join("windsurf"))
        }
        "zed" => {
            // Zed Editor: macOS/Linux ~/.config/zed/, Windows %APPDATA%/Zed/
            if cfg!(target_os = "windows") {
                Some(config_dir.join("Zed"))
            } else {
                Some(dirs::home_dir()?.join(".config").join("zed"))
            }
        }
        "continue_dev" => {
            // Continue.dev stores config at ~/.continue/ (cross-platform)
            Some(dirs::home_dir()?.join(".continue"))
        }
        _ => None,
    }
}

/// Format a user-friendly location label for a config path.
fn location_label(_platform: &str, base_dir: &PathBuf, relative: &str) -> String {
    let full = base_dir.join(relative);
    let display = full.display().to_string();

    // Use tildified path for readability
    if let Some(home) = dirs::home_dir() {
        let home_str = home.display().to_string();
        if display.starts_with(&home_str) {
            return format!("~{}", &display[home_str.len()..]);
        }
    }
    display
}

// ── Main discovery function ─────────────────────────────────────────────────

/// Discover all supported LLM platform config files on this machine.
///
/// Returns a list of `PlatformConfig` — one per known platform — in display
/// order.  Each entry includes whether the file was found, its current raw
/// content (if found), and the generated MCPOrb content (to be filled in
/// later by the caller).
pub fn discover_platform_configs() -> Vec<PlatformConfig> {
    let mut results = Vec::with_capacity(PLATFORMS.len());

    for platform in PLATFORMS {
        let base_dir = platform_config_dir(platform.platform);
        let config = match &base_dir {
            Some(dir) => {
                let config_path = dir.join(platform.relative_path);
                let loc_label = location_label(platform.platform, dir, platform.relative_path);
                let (exists, current_content, read_error) = if config_path.is_file() {
                    match fs::read_to_string(&config_path) {
                        Ok(content) => (true, Some(content), None),
                        Err(e) => {
                            let err = format!("Cannot read config: {e}");
                            (true, None, Some(err))
                        }
                    }
                } else {
                    (false, None, None)
                };

                PlatformConfig {
                    platform: platform.platform.to_string(),
                    display_name: platform.display_name.to_string(),
                    config_path: config_path.display().to_string(),
                    exists,
                    current_content,
                    read_error,
                    generated_content: None,
                    restart_hint: Some(platform.restart_hint_i18n_key.to_string()),
                    location_label: loc_label,
                }
            }
            None => {
                // Could not resolve base directory (unusual)
                PlatformConfig {
                    platform: platform.platform.to_string(),
                    display_name: platform.display_name.to_string(),
                    config_path: String::new(),
                    exists: false,
                    current_content: None,
                    read_error: Some("Could not resolve config directory for this OS".to_string()),
                    generated_content: None,
                    restart_hint: Some(platform.restart_hint_i18n_key.to_string()),
                    location_label: format!("({} not available on this OS)", platform.display_name),
                }
            }
        };
        results.push(config);
    }

    results
}

// ── Config content helpers ──────────────────────────────────────────────────

/// Build a complete `mcpServers` config JSON string that merges the user's
/// existing config with the MCPOrb entry.
///
/// If `existing_content` is `Some` and valid JSON, we parse it and try to
/// inject the MCPOrb entry into its `mcpServers` object, preserving all
/// existing entries. Otherwise we generate a fresh config from scratch.
///
/// `mcp_server_key` — the key to use inside `mcpServers` (e.g.
/// `"mcporb-my-orb"`).
/// `server_config` — a JSON value representing the server entry (e.g.
/// `{"command": "...", "args": [...]}`).
pub fn build_merged_config(
    existing_content: Option<&str>,
    mcp_server_key: &str,
    server_config: &Value,
) -> Result<String> {
    build_merged_config_with_entries(
        existing_content,
        &[(mcp_server_key.to_string(), server_config.clone())],
    )
}

/// Like `build_merged_config` but accepts multiple (`key`, `server_config`)
/// entries to inject at once. Preserves existing `mcpServers` entries, only
/// overwriting keys that collide with the provided entries.
pub fn build_merged_config_with_entries(
    existing_content: Option<&str>,
    entries: &[(String, Value)],
) -> Result<String> {
    let mut root: Value = match existing_content {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str(text).unwrap_or_else(|_| {
                serde_json::json!({ "mcpServers": {} })
            })
        }
        _ => serde_json::json!({ "mcpServers": {} }),
    };

    if !root.is_object() {
        root = serde_json::json!({ "mcpServers": {} });
    }
    if !root.get("mcpServers").map_or(false, |v| v.is_object()) {
        root["mcpServers"] = serde_json::json!({});
    }

    for (key, server_config) in entries {
        root["mcpServers"][key] = server_config.clone();
    }

    Ok(serde_json::to_string_pretty(&root)?)
}

/// Generate standard STDIO-based MCPOrb server config value.
pub fn make_stdio_server_config(runtime_binary: &str, _orb_id: &str, orb_zip_path: &str) -> Value {
    serde_json::json!({
        "command": runtime_binary,
        "args": [
            "--orb-zip",
            orb_zip_path,
            "--stdio-only"
        ]
    })
}

/// Generate HTTP-based MCPOrb server config value.
pub fn make_http_server_config(url: &str) -> Value {
    serde_json::json!({
        "url": url
    })
}

// ── Write config (with backup) ──────────────────────────────────────────────

/// Write a new config file, creating a `.bak` backup of the previous content
/// (if the file already existed).
pub fn write_platform_config(
    config_path: &str,
    new_content: &str,
) -> Result<WriteConfigResult> {
    let path = PathBuf::from(config_path);

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory {}", parent.display()))?;
    }

    // Create backup if file already exists
    let backup_path = if path.is_file() {
        let bak = path.with_extension("json.bak");
        fs::copy(&path, &bak)
            .with_context(|| format!("backup {} -> {}", path.display(), bak.display()))?;
        Some(bak.display().to_string())
    } else {
        None
    };

    // Write new content
    fs::write(&path, new_content)
        .with_context(|| format!("write {}", path.display()))?;

    Ok(WriteConfigResult {
        platform: String::new(), // filled in by caller
        success: true,
        config_path: path.display().to_string(),
        backup_path,
        error: None,
        restart_hint: None, // filled in by caller
    })
}

/// Read the current content of a platform config file (raw text).
pub fn read_config_raw(config_path: &str) -> Result<String> {
    fs::read_to_string(config_path).with_context(|| format!("read {}", config_path))
}

/// Pretty-print a JSON string (for display).
pub fn pretty_json(json_str: &str) -> Result<String> {
    let value: Value = serde_json::from_str(json_str)?;
    Ok(serde_json::to_string_pretty(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_returns_all_platforms() {
        let configs = discover_platform_configs();
        assert_eq!(configs.len(), PLATFORMS.len());
    }

    #[test]
    fn test_build_merged_config_from_empty() {
        let server = serde_json::json!({"command": "/usr/bin/mcporb", "args": ["--orb-zip", "test.zip"]});
        let result = build_merged_config(None, "mcporb-test", &server).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("mcpServers").is_some());
        assert!(parsed["mcpServers"].get("mcporb-test").is_some());
    }

    #[test]
    fn test_build_merged_config_preserves_existing() {
        let existing = r#"{"mcpServers": {"existing-orb": {"command": "old"}}}"#;
        let server = serde_json::json!({"command": "new-mcporb"});
        let result = build_merged_config(Some(existing), "mcporb-test", &server).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["mcpServers"].get("existing-orb").is_some());
        assert!(parsed["mcpServers"].get("mcporb-test").is_some());
    }

    #[test]
    fn test_write_and_readback() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("test_config.json");
        let path_str = config_path.display().to_string();

        // Write
        let result = write_platform_config(&path_str, "{\"key\": \"value\"}").unwrap();
        assert!(result.success);
        assert!(result.backup_path.is_none()); // no original to back up

        // Read back
        let content = read_config_raw(&path_str).unwrap();
        assert_eq!(content.trim(), "{\"key\": \"value\"}");

        // Write again — should create backup
        let result2 = write_platform_config(&path_str, "{\"key\": \"new_value\"}").unwrap();
        assert!(result2.backup_path.is_some());
        assert!(std::path::Path::new(&result2.backup_path.unwrap()).exists());
    }
}
