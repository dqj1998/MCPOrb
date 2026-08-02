//! End-to-end test: spawn the actual STDIO gateway binary with a mock
//! runtime and a real registry, then drive it like a real MCP client.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use mcporb_runtime_app_core::registry::InstallSource;
use mcporb_runtime_app_core::{InstalledOrb, OrbRegistry, RegistryStore};
use mcporb_runtime_core::format::{Capability, OrbManifest, RetrievalPlanKind};

/// Locate a workspace binary by name in the target directory.
/// Uses CARGO_MANIFEST_DIR (compile-time constant) to walk up to the workspace
/// root, then joins `target/<profile>/<binary_name>`.
fn bin_path(name: &str) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // crates/mcporb-gateway-core
    let workspace = manifest.parent().unwrap().parent().unwrap(); // workspace root
    let profile = option_env!("PROFILE").unwrap_or("debug");
    let target = workspace.join("target").join(profile).join(name);

    // Also try with the host's executable suffix
    if cfg!(target_os = "windows") && !target.extension().map(|e| e == "exe").unwrap_or(false) {
        let mut with_exe = target.clone();
        with_exe.set_extension("exe");
        if with_exe.exists() {
            return with_exe;
        }
    }

    target
}

fn gateway_bin() -> PathBuf { bin_path("mcporb-gateway-stdio") }
fn mock_runtime_bin() -> PathBuf { bin_path("mcporb-gateway-test-mock-runtime") }

fn make_manifest() -> OrbManifest {
    OrbManifest {
        name: "Mock Orb".into(),
        display_name: Some("Mock Orb".into()),
        version: "0.1.0".into(),
        description: "Mock orb for integration testing".into(),
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
    }
}

/// Set up a temporary registry with a single Orb pointing to the mock
/// runtime binary. Returns (temp_dir, registry_dir).
fn setup_registry() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let store = RegistryStore::new(dir.path().to_path_buf());

    let mock_bin = mock_runtime_bin();

    let installed = InstalledOrb {
        id: "mock-orb-id".into(),
        slug: "mock-orb".into(),
        display_name: "Mock Orb".into(),
        version: "0.1.0".into(),
        description: "Mock integration test orb".into(),
        manifest: make_manifest(),
        zip_path: mock_bin,
        zip_sha256: "mock_sha256".into(),
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

    let path = dir.path().to_path_buf();
    (dir, path)
}

fn spawn_gateway(registry_dir: &PathBuf) -> std::process::Child {
    Command::new(gateway_bin())
        .arg("--registry-dir")
        .arg(registry_dir)
        .arg("--runtime-binary")
        .arg(mock_runtime_bin())
        .arg("--idle-timeout")
        .arg("5")
        .arg("--check-interval")
        .arg("60")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn gateway binary")
}

/// Send a JSON-RPC line and read one response line from stdout.
fn send_and_recv(
    child_stdin: &mut impl Write,
    child_stdout: &mut std::io::BufReader<impl std::io::Read>,
    request: &serde_json::Value,
) -> serde_json::Value {
    let request_str = serde_json::to_string(request).unwrap();
    writeln!(child_stdin, "{request_str}").unwrap();
    child_stdin.flush().unwrap();

    let mut line = String::new();
    child_stdout.read_line(&mut line).unwrap();
    assert!(!line.is_empty(), "gateway closed stdout unexpectedly");
    serde_json::from_str(line.trim()).unwrap()
}

#[test]
fn stdio_gateway_initialize_and_list_tools() {
    let (_dir, registry_dir) = setup_registry();
    let mut child = spawn_gateway(&registry_dir);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());

    // 1. Initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "e2e-test", "version": "1.0" }
        }
    });
    let resp = send_and_recv(&mut stdin, &mut stdout, &init);
    assert_eq!(resp["result"]["serverInfo"]["name"], "MCPOrb Gateway");

    // 2. notifications/initialized
    let notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let notif_str = serde_json::to_string(&notif).unwrap();
    writeln!(stdin, "{notif_str}").unwrap();
    stdin.flush().unwrap();
    // no response expected for notification — give a tiny delay to avoid race
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 3. tools/list
    let list = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    let resp = send_and_recv(&mut stdin, &mut stdout, &list);
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1, "expected 1 namespaced tool from mock orb");
    assert_eq!(tools[0]["name"], "mock-orb__search_knowledge");

    // 4. ping
    let ping = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "ping"
    });
    let resp = send_and_recv(&mut stdin, &mut stdout, &ping);
    assert_eq!(resp["result"], serde_json::json!({}));

    // Clean shutdown
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn stdio_gateway_tool_call_routes_to_mock_orb() {
    let (_dir, registry_dir) = setup_registry();
    let mut child = spawn_gateway(&registry_dir);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());

    // Initialise first (required before tools/list or tools/call per MCP)
    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "e2e-test", "version": "1.0" }
        }
    });
    send_and_recv(&mut stdin, &mut stdout, &init);

    // Send notifications/initialized
    let notif = serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let notif_str = serde_json::to_string(&notif).unwrap();
    writeln!(stdin, "{notif_str}").unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Actually perform a tool call
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "mock-orb__search_knowledge",
            "arguments": { "query": "what is the meaning of life?" }
        }
    });
    let resp = send_and_recv(&mut stdin, &mut stdout, &request);
    assert!(resp.get("result").is_some(), "expected result, got: {resp}");
    let content = resp["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    assert!(
        text.contains("meaning of life"),
        "expected mock response to include query, got: {text}"
    );

    // Clean shutdown
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn stdio_gateway_batch_request() {
    let (_dir, registry_dir) = setup_registry();
    let mut child = spawn_gateway(&registry_dir);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());

    // Send a batch: [ping, tools/list]
    let batch = serde_json::json!([
        { "jsonrpc": "2.0", "id": 1, "method": "ping" },
        { "jsonrpc": "2.0", "id": 2, "method": "tools/list" }
    ]);
    let batch_str = serde_json::to_string(&batch).unwrap();
    writeln!(stdin, "{batch_str}").unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let responses: Vec<serde_json::Value> = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"], serde_json::json!({}));
    assert!(responses[1]["result"]["tools"].is_array());

    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn stdio_gateway_unknown_tool_returns_error() {
    let (_dir, registry_dir) = setup_registry();
    let mut child = spawn_gateway(&registry_dir);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "nonexistent__search_knowledge",
            "arguments": { "query": "test" }
        }
    });
    let resp = send_and_recv(&mut stdin, &mut stdout, &request);
    assert!(
        resp.get("error").is_some(),
        "expected error for unknown orb, got: {resp}"
    );

    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn stdio_gateway_resources_list() {
    let (_dir, registry_dir) = setup_registry();
    let mut child = spawn_gateway(&registry_dir);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/list"
    });
    let resp = send_and_recv(&mut stdin, &mut stdout, &request);
    let resources = resp["result"]["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["uri"], "orb://mock-orb/");

    child.kill().unwrap();
    child.wait().unwrap();
}
