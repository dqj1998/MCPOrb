//! Integration tests for `mcporb-gateway-core`.
//!
//! These tests exercise end-to-end flows:
//!   - Registry persistence + discovery round-trip
//!   - Handler dispatch with multi-orb configurations
//!   - Full MCP client lifecycle simulation

use serde_json::json;
use std::path::PathBuf;

use mcporb_gateway_core::handler::handle_request;
use mcporb_gateway_core::registry_reader::{
    discover_orbs_from_registry, GatewayConfig, GatewayOrb,
};
use mcporb_gateway_core::router::build_namespaced_tool_name;
use mcporb_gateway_core::runtime_manager::RuntimeManager;
use mcporb_gateway_core::GatewayTool;
use mcporb_runtime_app_core::{InstalledOrb, OrbRegistry, RegistryStore};
use mcporb_runtime_app_core::registry::InstallSource;
use mcporb_runtime_core::format::{Capability, OrbManifest, RetrievalPlanKind};

// ---------------------------------------------------------------------------
// Helper factories
// ---------------------------------------------------------------------------

fn make_gateway_orb(slug: &str, tool_count: usize) -> GatewayOrb {
    let tools: Vec<GatewayTool> = (0..tool_count)
        .map(|i| {
            let method = if i == 0 {
                "search_knowledge".to_string()
            } else {
                format!("tool_{i}")
            };
            GatewayTool {
                original_name: method.clone(),
                namespaced_name: build_namespaced_tool_name(slug, &method),
                description: format!("{slug} tool {i}"),
                input_schema: json!({}),
            }
        })
        .collect();

    GatewayOrb {
        id: format!("{slug}_id"),
        slug: slug.to_string(),
        display_name: slug.to_string(),
        description: format!("{slug} test orb"),
        zip_path: PathBuf::from(format!("/tmp/orbs/{slug}.zip")),
        mcp_protocol_version: "2024-11-05".to_string(),
        tools,
    }
}

fn make_manager(orbs: Vec<GatewayOrb>) -> RuntimeManager {
    let config = GatewayConfig::default();
    RuntimeManager::new(config, orbs)
}

fn dummy_orb_manifest() -> OrbManifest {
    OrbManifest {
        name: "Dummy".into(),
        display_name: None,
        version: "0.1.0".into(),
        description: ".".into(),
        orb_format_version: "0.2".into(),
        runtime_min_version: None,
        builder_version: None,
        mcp_protocol_version: "2024-11-05".into(),
        build_time: "2026-07-01T00:00:00Z".into(),
        created_at: None,
        source_documents: vec![],
        chunk_count: 0,
        index_format_version: "0.2".into(),
        binary_size_target_mb: 20,
        assets_sha256: None,
        encrypted: false,
        selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
        enabled_capabilities: vec![Capability::Bm25],
        embedding_dim: None,
        embedding_model: None,
        embedding_model_tar_sha256: None,
        trigram_min_df: None,
        planning_rationale: vec![],
    }
}

fn dummy_installed_orb() -> InstalledOrb {
    InstalledOrb {
        id: "dummy".into(),
        slug: "dummy".into(),
        display_name: "Dummy".into(),
        version: "0.1.0".into(),
        description: ".".into(),
        manifest: dummy_orb_manifest(),
        zip_path: PathBuf::from("/tmp/orbs/dummy.zip"),
        zip_sha256: "abc".into(),
        assets_sha256: String::new(),
        install_source: InstallSource::LocalImport,
        store_artifact_id: None,
        encrypted_assets: false,
        password_protected: false,
        password_persistence: None,
        last_used_at: None,
    }
}

// ---------------------------------------------------------------------------
// Registry persistence tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn registry_roundtrip_single_orb() {
    let dir = tempfile::tempdir().unwrap();
    let store = RegistryStore::new(dir.path().to_path_buf());

    let installed = InstalledOrb {
        id: "test-id".into(),
        slug: "my-orb".into(),
        display_name: "My Orb".into(),
        version: "0.1.0".into(),
        description: "A test orb".into(),
        manifest: OrbManifest {
            name: "My Orb".into(),
            display_name: Some("My Orb".into()),
            version: "0.1.0".into(),
            description: "A test orb".into(),
            orb_format_version: "0.2".into(),
            runtime_min_version: None,
            builder_version: None,
            mcp_protocol_version: "2024-11-05".into(),
            build_time: "2026-07-01T00:00:00Z".into(),
            created_at: None,
            source_documents: vec!["doc.pdf".into()],
            chunk_count: 100,
            index_format_version: "0.2".into(),
            binary_size_target_mb: 20,
            assets_sha256: None,
            encrypted: false,
            selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
            enabled_capabilities: vec![Capability::Bm25],
            embedding_dim: None,
            embedding_model: None,
            embedding_model_tar_sha256: None,
            trigram_min_df: None,
            planning_rationale: vec![],
        },
        zip_path: PathBuf::from("/tmp/orbs/my-orb.zip"),
        zip_sha256: "abc123".into(),
        assets_sha256: String::new(),
        install_source: InstallSource::LocalImport,
        store_artifact_id: None,
        encrypted_assets: false,
        password_protected: false,
        password_persistence: None,
        last_used_at: None,
    };

    let mut registry = OrbRegistry::default();
    registry.orbs.push(installed);
    store.save(&registry).unwrap();

    let orbs = discover_orbs_from_registry(&store).unwrap();
    assert_eq!(orbs.len(), 1);
    assert_eq!(orbs[0].slug, "my-orb");
    assert_eq!(orbs[0].id, "test-id");
}

#[tokio::test]
async fn registry_roundtrip_multiple_orbs() {
    let dir = tempfile::tempdir().unwrap();
    let store = RegistryStore::new(dir.path().to_path_buf());

    let mut registry = OrbRegistry::default();
    for i in 0..5 {
        registry.orbs.push(InstalledOrb {
            id: format!("id-{i}"),
            slug: format!("orb-{i}"),
            display_name: format!("Orb {i}"),
            ..dummy_installed_orb()
        });
    }
    store.save(&registry).unwrap();

    let orbs = discover_orbs_from_registry(&store).unwrap();
    assert_eq!(orbs.len(), 5);
    for (i, orb) in orbs.iter().enumerate() {
        assert_eq!(orb.slug, format!("orb-{i}"));
    }
}

#[tokio::test]
async fn registry_roundtrip_roundtrips_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = RegistryStore::new(dir.path().to_path_buf());

    let installed = InstalledOrb {
        id: "id-42".into(),
        slug: "test-slug".into(),
        display_name: "Test Display".into(),
        version: "2.1.0".into(),
        description: "A comprehensive test".into(),
        manifest: dummy_orb_manifest(),
        zip_path: PathBuf::from("/tmp/orbs/my-orb.zip"),
        zip_sha256: "abcdef".into(),
        assets_sha256: "deadbeef".into(),
        install_source: InstallSource::StoreDownload,
        store_artifact_id: Some("store-abc".into()),
        encrypted_assets: true,
        password_protected: true,
        password_persistence: Some("device".into()),
        last_used_at: Some("2026-07-12T10:00:00Z".into()),
    };

    let mut registry = OrbRegistry::default();
    registry.orbs.push(installed);
    store.save(&registry).unwrap();

    let orbs = discover_orbs_from_registry(&store).unwrap();
    assert_eq!(orbs.len(), 1);
    assert_eq!(orbs[0].id, "id-42");
    assert_eq!(orbs[0].slug, "test-slug");
    assert_eq!(orbs[0].display_name, "Test Display");
}

// ---------------------------------------------------------------------------
// Handler integration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn handler_with_5_orbs_tools_list_all_namespaced() {
    let orbs: Vec<GatewayOrb> = (0..5)
        .map(|i| make_gateway_orb(&format!("orb-{i}"), 3))
        .collect();
    let manager = make_manager(orbs);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    let response = handle_request(&manager, request)
        .await
        .unwrap()
        .unwrap();
    let tools = response["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 15); // 5 orbs × 3 tools each

    for i in 0..5 {
        let expected = build_namespaced_tool_name(&format!("orb-{i}"), "search_knowledge");
        assert!(
            tools.iter().any(|t| t["name"] == expected),
            "missing {expected} in tool list"
        );
    }
}

#[tokio::test]
async fn handler_resource_list_per_orb() {
    let orbs: Vec<GatewayOrb> = (0..3)
        .map(|i| make_gateway_orb(&format!("orb-{i}"), 1))
        .collect();
    let manager = make_manager(orbs);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/list"
    });

    let response = handle_request(&manager, request)
        .await
        .unwrap()
        .unwrap();
    let resources = response["result"]["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 3);
    assert!(resources.iter().any(|r| r["uri"] == "orb://orb-0/"));
    assert!(resources.iter().any(|r| r["uri"] == "orb://orb-1/"));
    assert!(resources.iter().any(|r| r["uri"] == "orb://orb-2/"));
}

#[tokio::test]
async fn handler_ping_returns_empty() {
    let manager = make_manager(vec![]);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ping"
    });

    let response = handle_request(&manager, request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response["result"], json!({}));
}

#[tokio::test]
async fn handler_invalid_format_returns_error() {
    let manager = make_manager(vec![]);

    let request = json!("bad");
    let response = handle_request(&manager, request)
        .await
        .unwrap()
        .unwrap();
    assert!(response.get("error").is_some());
}

#[tokio::test]
async fn handler_tool_call_missing_params() {
    let orbs = vec![make_gateway_orb("test-orb", 1)];
    let manager = make_manager(orbs);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call"
    });

    let response = handle_request(&manager, request)
        .await
        .unwrap()
        .unwrap();
    assert!(response.get("error").is_some());
}

#[tokio::test]
async fn handler_resource_read_missing_params() {
    let orbs = vec![make_gateway_orb("test-orb", 1)];
    let manager = make_manager(orbs);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/read"
    });

    let response = handle_request(&manager, request)
        .await
        .unwrap()
        .unwrap();
    assert!(response.get("error").is_some());
}

// ---------------------------------------------------------------------------
// RuntimeManager integration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runtime_manager_100_orbs_stress() {
    let orbs: Vec<GatewayOrb> = (0..100)
        .map(|i| make_gateway_orb(&format!("orb-{i}"), 1))
        .collect();
    let manager = make_manager(orbs);

    assert_eq!(manager.list_orbs().len(), 100);
    assert!(manager.has_orb("orb-0"));
    assert!(manager.has_orb("orb-99"));
    assert!(!manager.has_orb("orb-100"));

    let statuses = manager.list_orb_statuses().await;
    assert_eq!(statuses.len(), 100);
    for (_, status) in &statuses {
        assert_eq!(*status, "idle");
    }
}

#[tokio::test]
async fn runtime_manager_default_config_contains_runtime_dir() {
    let config = GatewayConfig::default();
    let path = config.registry_dir.to_string_lossy();
    assert!(
        path.contains("MCPOrb") && path.contains("Runtime"),
        "expected config path to contain MCPOrb/Runtime, got: {path}"
    );
}

// ---------------------------------------------------------------------------
// MCP client lifecycle simulation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_mcp_lifecycle_initialize_then_tools_list() {
    let orbs: Vec<GatewayOrb> = (0..2)
        .map(|i| make_gateway_orb(&format!("orb-{i}"), 2))
        .collect();
    let manager = make_manager(orbs);

    // Step 1: Initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }
    });
    let init_resp = handle_request(&manager, init_req)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        init_resp["result"]["serverInfo"]["name"],
        "MCPOrb Gateway"
    );

    // Step 2: tools/list (4 tools: 2 orbs × 2 tools)
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    let list_resp = handle_request(&manager, list_req)
        .await
        .unwrap()
        .unwrap();
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
}

#[tokio::test]
async fn handler_batch_mixed_methods() {
    let orbs = vec![make_gateway_orb("my-orb", 1)];
    let manager = make_manager(orbs);

    let batch = json!([
        { "jsonrpc": "2.0", "id": 1, "method": "ping" },
        { "jsonrpc": "2.0", "id": 2, "method": "tools/list" },
        { "jsonrpc": "2.0", "method": "notifications/initialized" }
    ]);

    let response = handle_request(&manager, batch)
        .await
        .unwrap()
        .unwrap();
    let responses = response.as_array().unwrap();

    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"], json!({}));
    assert!(responses[1]["result"]["tools"].is_array());
}

#[tokio::test]
async fn handle_request_with_mcp_client_lifecycle() {
    // Simulate: initialize → notifications/initialized → tools/list →
    // tools/call (bad namespace) → resources/list → ping
    let orbs: Vec<GatewayOrb> = (0..3)
        .map(|i| make_gateway_orb(&format!("kb-{i}"), 1))
        .collect();
    let manager = make_manager(orbs);

    // 1. initialize
    let resp = handle_request(
        &manager,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {} }
        }),
    )
    .await
    .unwrap();
    assert!(resp.is_some());
    assert_eq!(resp.unwrap()["result"]["protocolVersion"], "2024-11-05");

    // 2. notifications/initialized
    let resp = handle_request(
        &manager,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await
    .unwrap();
    assert!(resp.is_none());

    // 3. tools/list
    let resp = handle_request(
        &manager,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(resp["result"]["tools"].as_array().unwrap().len(), 3);

    // 4. tools/call with bad namespace
    let resp = handle_request(
        &manager,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "nonexistent__search_knowledge", "arguments": { "query": "hi" } }
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(resp.get("error").is_some());

    // 5. resources/list
    let resp = handle_request(
        &manager,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/list" }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(resp["result"]["resources"].as_array().unwrap().len(), 3);

    // 6. ping
    let resp = handle_request(
        &manager,
        json!({ "jsonrpc": "2.0", "id": 5, "method": "ping" }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(resp["result"], json!({}));
}
