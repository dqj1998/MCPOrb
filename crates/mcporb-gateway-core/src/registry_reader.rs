//! Read the MCPOrb registry and build the `GatewayOrb` list.
//!
//! The gateway reads from the same `registry.json` that the Tauri app
//! maintains — zero sync cost, always consistent.

use std::path::PathBuf;

use anyhow::Result;
use mcporb_runtime_app_core::{InstalledOrb, RegistryStore};
use tracing;

use crate::router;
use crate::GatewayTool;

/// A single Orb as seen by the gateway.
#[derive(Debug, Clone)]
pub struct GatewayOrb {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub zip_path: PathBuf,
    pub mcp_protocol_version: String,
    pub tools: Vec<GatewayTool>,
}

/// Configuration for the gateway.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
/// Directory containing `registry.json` (the orb index). Orb ZIPs are NOT
/// resolved relative to this directory — each `InstalledOrb.zip_path` in the
/// registry is an absolute path to the ZIP. Platform note: on Windows/Linux
/// (and macOS with the default layout) ZIPs live at `<registry_dir>/Orbs/`,
/// but when a macOS user picks a custom Orb library folder, ZIPs move to
/// `<library>/Orbs` and this directory holds only the index.
pub registry_dir: PathBuf,
    /// Path to the `mcporb-runtime` binary used to spawn Orb child processes.
    pub runtime_binary: PathBuf,
    /// Seconds of inactivity before an Orb child process is killed.
    pub idle_timeout_secs: u64,
    /// Seconds between idle-reaper checks.
    pub check_interval_secs: u64,
    /// TCP port for the HTTP gateway (if applicable).
    pub http_port: u16,
    /// Transport label recorded in child process metrics ("stdio" or "http").
    /// Set to "stdio" for the STDIO gateway and "http" for the HTTP gateway.
    pub mcp_transport: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        let registry_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("MCPOrb")
            .join("Runtime");

        // Try to find mcporb-runtime next to the running binary, or fall back.
        let runtime_binary = default_runtime_binary().unwrap_or_else(|| {
            registry_dir
                .parent()
                .map(|p| p.join("mcporb-runtime"))
                .unwrap_or_else(|| PathBuf::from("mcporb-runtime"))
        });

        Self {
            registry_dir,
            runtime_binary,
            idle_timeout_secs: 300,    // 5 minutes
            check_interval_secs: 30,
            http_port: 5600,
            mcp_transport: "stdio".to_string(),
        }
    }
}

/// Discover all installed Orbs by reading the registry.
///
/// Returns `Ok(vec![])` when the registry file doesn't exist (fresh install).
pub fn discover_orbs(config: &GatewayConfig) -> Result<Vec<GatewayOrb>> {
    let store = RegistryStore::new(config.registry_dir.clone());
    let registry = store.load().unwrap_or_default();

    let mut orbs = Vec::with_capacity(registry.orbs.len());
    for installed in &registry.orbs {
        let orb = build_gateway_orb(installed)?;
        orbs.push(orb);
    }
    Ok(orbs)
}

/// Read orbs from an explicit `RegistryStore` (useful for testing).
pub fn discover_orbs_from_registry(store: &RegistryStore) -> Result<Vec<GatewayOrb>> {
    let registry = store.load().unwrap_or_default();
    let mut orbs = Vec::with_capacity(registry.orbs.len());
    for installed in &registry.orbs {
        let orb = build_gateway_orb(installed)?;
        orbs.push(orb);
    }
    Ok(orbs)
}

/// Build a `GatewayOrb` from a registry `InstalledOrb`.
fn build_gateway_orb(installed: &InstalledOrb) -> Result<GatewayOrb> {
    let tools = build_tools_from_manifest(installed);

    Ok(GatewayOrb {
        id: installed.id.clone(),
        slug: installed.slug.clone(),
        display_name: installed.display_name.clone(),
        description: installed.description.clone(),
        zip_path: installed.zip_path.clone(),
        mcp_protocol_version: installed.manifest.mcp_protocol_version.clone(),
        tools,
    })
}

/// Claude Desktop enforces tool names ≤ 64 chars; truncate slug to ensure
/// `{slug}__search_knowledge` fits.
const MAX_MCP_TOOL_NAME_LEN: usize = 64;

/// Build the `GatewayTool` list from the installed orb's manifest.
///
/// All Orbs expose a `search_knowledge` tool (with appropriate description
/// and available search methods based on the manifest's capabilities).
fn build_tools_from_manifest(installed: &InstalledOrb) -> Vec<GatewayTool> {
    let slug = &installed.slug;
    let manifest = &installed.manifest;

    let namespaced_suffix_len = crate::router::NAMESPACE_SEP.len() + "search_knowledge".len();
    let max_slug_for_mcp = MAX_MCP_TOOL_NAME_LEN.saturating_sub(namespaced_suffix_len);
    let truncated_slug: &str = if slug.len() > max_slug_for_mcp {
        tracing::warn!(
            orb = %slug,
            slug_len = slug.len(),
            max_slug_len = max_slug_for_mcp,
            "Orb slug too long for MCP tool name (max {max_slug_for_mcp} chars), truncating"
        );
        &slug[..max_slug_for_mcp]
    } else {
        slug
    };

    // Build the search method enum from enabled capabilities
    let method_enum: Vec<&str> = {
        let mut methods = vec!["auto"];
        if manifest
            .enabled_capabilities
            .iter()
            .any(|c| matches!(c, mcporb_runtime_core::format::Capability::Bm25))
        {
            methods.push("bm25");
        }
        if manifest
            .enabled_capabilities
            .iter()
            .any(|c| matches!(c, mcporb_runtime_core::format::Capability::TfIdf))
        {
            methods.push("tfidf");
        }
        if manifest
            .enabled_capabilities
            .iter()
            .any(|c| matches!(c, mcporb_runtime_core::format::Capability::Trigram))
        {
            methods.push("trigram");
        }
        if manifest
            .enabled_capabilities
            .iter()
            .any(|c| {
                matches!(c, mcporb_runtime_core::format::Capability::FlatVector)
                    || matches!(c, mcporb_runtime_core::format::Capability::Hnsw)
            })
        {
            methods.push("vector");
            methods.push("hybrid");
        }
        methods
    };

    let method_description = build_method_description(&method_enum);

    let namespaced_tool_name = router::build_namespaced_tool_name(truncated_slug, "search_knowledge");

    vec![GatewayTool {
        original_name: "search_knowledge".to_string(),
        namespaced_name: namespaced_tool_name,
        description: format!(
            "Search the {} knowledge base using semantic or keyword retrieval. \
             Returns ranked chunks with source attribution and relevance scores.",
            manifest.name
        ),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query — a natural-language question or a set of keywords to find relevant knowledge."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Number of top results to return (default: 5, max: 50).",
                    "default": 5
                },
                "method": {
                    "type": "string",
                    "description": method_description,
                    "enum": method_enum,
                    "default": "auto"
                }
            },
            "required": ["query"]
        }),
    }]
}

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

/// Locate `mcporb-runtime` binary relative to the current executable.
fn default_runtime_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let suffix = std::env::consts::EXE_SUFFIX;
    for name in ["mcporb-runtime", "mcporb-runtime-lite"] {
        let path = dir.join(format!("{name}{suffix}"));
        if path.is_file() {
            return Some(path);
        }
    }
    Some(dir.join(format!("mcporb-runtime{suffix}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcporb_runtime_app_core::{InstalledOrb, OrbRegistry};
    use mcporb_runtime_app_core::registry::InstallSource;
    use mcporb_runtime_core::format::{Capability, OrbManifest, RetrievalPlanKind};
    use std::path::Path;

    fn make_manifest(name: &str, capabilities: Vec<Capability>) -> OrbManifest {
        OrbManifest {
            name: name.to_string(),
            display_name: Some(name.to_string()),
            version: "0.1.0".to_string(),
            description: format!("{name} knowledge base"),
            orb_format_version: "0.2".to_string(),
            runtime_min_version: None,
            builder_version: None,
            mcp_protocol_version: "2024-11-05".to_string(),
            build_time: "2026-07-01T00:00:00Z".to_string(),
            created_at: None,
            source_documents: vec!["doc.pdf".to_string()],
            chunk_count: 100,
            index_format_version: "0.2".to_string(),
            binary_size_target_mb: 20,
            assets_sha256: None,
            encrypted: false,
            selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
            enabled_capabilities: capabilities,
            embedding_dim: None,
            embedding_model: None,
            embedding_model_tar_sha256: None,
            trigram_min_df: None,
            planning_rationale: vec![],
        }
    }

    fn make_installed_orb(
        id: &str,
        slug: &str,
        name: &str,
        capabilities: Vec<Capability>,
    ) -> InstalledOrb {
        InstalledOrb {
            id: id.to_string(),
            slug: slug.to_string(),
            display_name: name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("{name} knowledge base"),
            manifest: make_manifest(name, capabilities),
            zip_path: PathBuf::from(format!("/tmp/orbs/{id}.zip")),
            zip_sha256: format!("{id}_sha256"),
            assets_sha256: String::new(),
            install_source: InstallSource::LocalImport,
            store_artifact_id: None,
            encrypted_assets: false,
            password_protected: false,
            password_persistence: None,
            last_used_at: None,
        }
    }

    #[test]
    fn discover_from_empty_registry() {
        let dir = tempfile::tempdir().unwrap();
        let store = RegistryStore::new(dir.path().to_path_buf());
        let orbs = discover_orbs_from_registry(&store).unwrap();
        assert!(orbs.is_empty());
    }

    #[test]
    fn discover_single_orb() {
        let dir = tempfile::tempdir().unwrap();
        let store = RegistryStore::new(dir.path().to_path_buf());
        let installed = make_installed_orb("id1", "test-orb", "Test Orb", vec![Capability::Bm25]);

        let mut registry = OrbRegistry::default();
        registry.orbs.push(installed);
        store.save(&registry).unwrap();

        let orbs = discover_orbs_from_registry(&store).unwrap();
        assert_eq!(orbs.len(), 1);
        assert_eq!(orbs[0].slug, "test-orb");
        assert_eq!(orbs[0].display_name, "Test Orb");
        assert_eq!(orbs[0].zip_path, Path::new("/tmp/orbs/id1.zip"));
    }

    #[test]
    fn discover_multiple_orbs() {
        let dir = tempfile::tempdir().unwrap();
        let store = RegistryStore::new(dir.path().to_path_buf());

        let mut registry = OrbRegistry::default();
        registry
            .orbs
            .push(make_installed_orb("id1", "orb-one", "Orb One", vec![Capability::Bm25]));
        registry
            .orbs
            .push(make_installed_orb("id2", "orb-two", "Orb Two", vec![Capability::Bm25]));
        store.save(&registry).unwrap();

        let orbs = discover_orbs_from_registry(&store).unwrap();
        assert_eq!(orbs.len(), 2);
        assert_eq!(orbs[0].slug, "orb-one");
        assert_eq!(orbs[1].slug, "orb-two");
    }

    #[test]
    fn tool_name_is_namespaced() {
        let installed =
            make_installed_orb("id1", "my-orb", "My Orb", vec![Capability::Bm25]);
        let gateway_orb = build_gateway_orb(&installed).unwrap();

        assert_eq!(gateway_orb.tools.len(), 1);
        assert_eq!(gateway_orb.tools[0].namespaced_name, "my-orb__search_knowledge");
        assert_eq!(gateway_orb.tools[0].original_name, "search_knowledge");
    }

    #[test]
    fn tool_schema_includes_method_enum() {
        let installed = make_installed_orb(
            "id1",
            "my-orb",
            "My Orb",
            vec![Capability::Bm25, Capability::FlatVector],
        );
        let gateway_orb = build_gateway_orb(&installed).unwrap();

        let method_enum = gateway_orb.tools[0]
            .input_schema
            .get("properties")
            .and_then(|p| p.get("method"))
            .and_then(|m| m.get("enum"))
            .and_then(|e| e.as_array())
            .unwrap();

        let methods: Vec<&str> = method_enum
            .iter()
            .filter_map(|v| v.as_str())
            .collect();

        assert!(methods.contains(&"auto"));
        assert!(methods.contains(&"bm25"));
        assert!(methods.contains(&"vector"));
        assert!(methods.contains(&"hybrid"));
        assert!(!methods.contains(&"tfidf")); // not enabled
    }

    #[test]
    fn tool_schema_requires_query() {
        let installed =
            make_installed_orb("id1", "my-orb", "My Orb", vec![Capability::Bm25]);
        let gateway_orb = build_gateway_orb(&installed).unwrap();

        let required = gateway_orb.tools[0]
            .input_schema
            .get("required")
            .and_then(|r| r.as_array())
            .unwrap();

        assert!(required.iter().any(|v| v == "query"));
    }

    #[test]
    fn tool_description_contains_orb_name() {
        let installed =
            make_installed_orb("id1", "my-orb", "My Orb", vec![Capability::Bm25]);
        let gateway_orb = build_gateway_orb(&installed).unwrap();

        assert!(gateway_orb.tools[0].description.contains("My Orb"));
    }

    #[test]
    fn long_slug_is_truncated_to_fit_tool_name_limit() {
        // Slug long enough that {slug}__search_knowledge exceeds 64 chars
        let long_slug = "a".repeat(50);
        let installed = make_installed_orb("id1", &long_slug, "Long Slug Orb", vec![Capability::Bm25]);
        let gateway_orb = build_gateway_orb(&installed).unwrap();

        let namespaced = &gateway_orb.tools[0].namespaced_name;
        // Must not exceed 64 chars
        assert!(namespaced.len() <= 64, "namespaced name length {} exceeds 64: {namespaced}", namespaced.len());
        // Must still end with __search_knowledge
        assert!(namespaced.ends_with("__search_knowledge"), "namespaced name does not end with __search_knowledge: {namespaced}");
        // Slug portion must be 46 chars (64 - 2 for __ - 16 for search_knowledge = 46)
        let slug_part = namespaced.trim_end_matches("__search_knowledge");
        assert_eq!(slug_part.len(), 46, "slug part should be truncated to 46 chars, got {}: {slug_part}", slug_part.len());
    }

    #[test]
    fn short_slug_is_not_truncated() {
        let installed =
            make_installed_orb("id1", "short-slug", "Short Slug", vec![Capability::Bm25]);
        let gateway_orb = build_gateway_orb(&installed).unwrap();
        assert_eq!(
            gateway_orb.tools[0].namespaced_name,
            "short-slug__search_knowledge"
        );
    }
}
