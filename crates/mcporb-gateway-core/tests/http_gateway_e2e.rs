//! End-to-end test: spawn the HTTP gateway binary and drive it like a
//! real MCP client over HTTP.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use mcporb_runtime_app_core::registry::InstallSource;
use mcporb_runtime_app_core::{InstalledOrb, OrbRegistry, RegistryStore};
use mcporb_runtime_core::format::{Capability, OrbManifest, RetrievalPlanKind};

fn bin_path(name: &str) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent().unwrap().parent().unwrap();
    let profile = option_env!("PROFILE").unwrap_or("debug");
    workspace.join("target").join(profile).join(name)
}

fn gateway_bin() -> PathBuf { bin_path("mcporb-gateway-http") }
fn mock_runtime_bin() -> PathBuf { bin_path("mcporb-gateway-test-mock-runtime") }

fn make_manifest() -> OrbManifest {
    OrbManifest {
        name: "Mock Orb".into(),
        display_name: Some("Mock Orb".into()),
        version: "0.1.0".into(),
        description: "Mock orb for HTTP gateway integration test".into(),
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

struct HttpGatewayHarness {
    child: Child,
    base_url: String,
    _dir: tempfile::TempDir,
}

impl HttpGatewayHarness {
    async fn start(port: u16) -> Self {
        Self::start_impl(port, None).await
    }

    async fn start_with_token(port: u16, token: &str) -> Self {
        Self::start_impl(port, Some(token)).await
    }

    async fn start_impl(port: u16, token: Option<&str>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = RegistryStore::new(dir.path().to_path_buf());

        let mock_bin = mock_runtime_bin();
        let installed = InstalledOrb {
            id: "mock-orb-id".into(),
            slug: "mock-orb".into(),
            display_name: "Mock Orb".into(),
            version: "0.1.0".into(),
            description: "Mock".into(),
            manifest: make_manifest(),
            zip_path: mock_bin,
            zip_sha256: "sha".into(),
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

        let mut cmd = Command::new(gateway_bin());
        cmd.arg("--registry-dir").arg(dir.path())
            .arg("--runtime-binary").arg(mock_runtime_bin())
            .arg("--port").arg(port.to_string())
            .arg("--idle-timeout").arg("5")
            .arg("--check-interval").arg("60");
        if let Some(t) = token {
            cmd.arg("--token").arg(t);
        }
        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn HTTP gateway");

        let harness = Self {
            child,
            base_url: format!("http://127.0.0.1:{port}"),
            _dir: dir,
        };
        harness.wait_ready(token).await;
        harness
    }

    async fn wait_ready(&self, token: Option<&str>) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let url = format!("{}/mcp", self.base_url);
        let ping = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "ping"
        });

        for i in 0..20 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let mut req = client.post(&url).json(&ping);
            if let Some(t) = token {
                req = req.bearer_auth(t);
            }
            match req.send().await {
                Ok(resp) if resp.status().is_success() => return,
                _ if i < 19 => continue,
                _ => panic!("HTTP gateway did not become ready after 5s"),
            }
        }
    }

    async fn post_mcp(&self, body: &serde_json::Value) -> reqwest::Response {
        self.post_mcp_authed(body, None).await
    }

    async fn post_mcp_authed(
        &self,
        body: &serde_json::Value,
        token: Option<&str>,
    ) -> reqwest::Response {
        let client = reqwest::Client::new();
        let mut req = client.post(format!("{}/mcp", self.base_url)).json(body);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        req.send()
            .await
            .expect("HTTP request to gateway failed")
    }
}

impl Drop for HttpGatewayHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

const PORT_BASE: u16 = 19890;

#[tokio::test]
async fn http_gateway_initialize() {
    let harness = HttpGatewayHarness::start(PORT_BASE).await;
    let resp = harness.post_mcp(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "http-test", "version": "1.0" }
        }
    })).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "MCPOrb Gateway");
}

#[tokio::test]
async fn http_gateway_tools_list() {
    let harness = HttpGatewayHarness::start(PORT_BASE + 1).await;
    let resp = harness.post_mcp(&serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list"
    })).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "mock-orb__search_knowledge");
}

#[tokio::test]
async fn http_gateway_ping() {
    let harness = HttpGatewayHarness::start(PORT_BASE + 2).await;
    let resp = harness.post_mcp(&serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "ping"
    })).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], serde_json::json!({}));
}

#[tokio::test]
async fn http_gateway_resources_list() {
    let harness = HttpGatewayHarness::start(PORT_BASE + 3).await;
    let resp = harness.post_mcp(&serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "resources/list"
    })).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    let resources = body["result"]["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["uri"], "orb://mock-orb/");
}

#[tokio::test]
async fn http_gateway_unknown_method() {
    let harness = HttpGatewayHarness::start(PORT_BASE + 4).await;
    let resp = harness.post_mcp(&serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "unknown_method"
    })).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn http_gateway_tool_call_bad_namespace() {
    let harness = HttpGatewayHarness::start(PORT_BASE + 5).await;
    let resp = harness.post_mcp(&serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "nonexistent__search_knowledge", "arguments": { "query": "test" } }
    })).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn http_gateway_notification_returns_accepted() {
    let harness = HttpGatewayHarness::start(PORT_BASE + 6).await;
    let resp = harness.post_mcp(&serde_json::json!({
        "jsonrpc": "2.0", "method": "notifications/initialized"
    })).await;
    assert_eq!(resp.status().as_u16(), 202);
}

#[tokio::test]
async fn http_gateway_batch_request() {
    let harness = HttpGatewayHarness::start(PORT_BASE + 7).await;
    let resp = harness.post_mcp(&serde_json::json!([
        { "jsonrpc": "2.0", "id": 1, "method": "ping" },
        { "jsonrpc": "2.0", "id": 2, "method": "tools/list" }
    ])).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    let responses = body.as_array().unwrap();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"], serde_json::json!({}));
}

#[tokio::test]
async fn http_gateway_with_auth_rejects_missing_and_wrong_token() {
    let harness = HttpGatewayHarness::start_with_token(PORT_BASE + 8, "e2e-secret").await;
    let ping = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    // No credential → 401.
    let resp = harness.post_mcp(&ping).await;
    assert_eq!(resp.status().as_u16(), 401);

    // Wrong bearer token → 401.
    let resp = harness.post_mcp_authed(&ping, Some("wrong")).await;
    assert_eq!(resp.status().as_u16(), 401);

    // Correct bearer token → success.
    let resp = harness.post_mcp_authed(&ping, Some("e2e-secret")).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], serde_json::json!({}));
}
