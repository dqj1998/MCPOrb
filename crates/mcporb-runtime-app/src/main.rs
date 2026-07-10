#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use mcporb_runtime_app_core::{
    mcp_config, metrics, platform_config, search, settings::SettingsStore, store_client::StoreClient,
    ImportOptions, ImportResult, InstalledOrb, PlatformConfig, RegistryStore, RuntimeSettings,
    SearchResponse, StoreOrb, StoreSearchResult, WriteConfigResult,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    registry: RegistryStore,
    settings: Arc<Mutex<SettingsStore>>,
    running_orbs: Arc<Mutex<Vec<RunningOrb>>>,
    running_children: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunningOrb {
    orb_id: String,
    slug: String,
    port: u16,
    token: String,
    pid: u32,
}

#[derive(Debug, Serialize)]
struct RuntimeStatus {
    version: &'static str,
}

#[tauri::command]
fn runtime_status() -> Result<RuntimeStatus, String> {
    Ok(RuntimeStatus {
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tauri::command]
fn list_orbs(state: tauri::State<AppState>) -> Result<Vec<InstalledOrb>, String> {
    state.registry.list().map_err(to_string)
}

#[tauri::command]
fn import_orb_zip(path: String, state: tauri::State<AppState>) -> Result<ImportResult, String> {
    state
        .registry
        .import_zip(&PathBuf::from(path), ImportOptions::default())
        .map_err(to_string)
}

#[tauri::command]
fn search_orb(
    orb_id: String,
    query: String,
    method: Option<String>,
    top_k: Option<usize>,
    state: tauri::State<AppState>,
) -> Result<SearchResponse, String> {
    let orb = state
        .registry
        .get(&orb_id)
        .map_err(to_string)?
        .ok_or_else(|| format!("Orb `{orb_id}` is not installed"))?;
    search::search_zip(&orb.zip_path, &query, method.as_deref(), top_k).map_err(to_string)
}

#[tauri::command]
fn mcp_config_snippets(
    orb_id: String,
    runtime_binary: Option<String>,
    state: tauri::State<AppState>,
) -> Result<Vec<mcp_config::McpConfigSnippet>, String> {
    let orb = state
        .registry
        .get(&orb_id)
        .map_err(to_string)?
        .ok_or_else(|| format!("Orb `{orb_id}` is not installed"))?;
    let use_wrapper = is_runner_wrapper_mode();
    let binary = || -> Option<PathBuf> {
        if let Some(b) = runtime_binary {
            return Some(PathBuf::from(b));
        }
        if use_wrapper { mcp_binary_path() } else { default_runtime_binary() }
    };
    let runtime_path = binary()
        .ok_or_else(|| "Could not resolve mcporb-runtime binary path".to_string())?;
    Ok(mcp_config::stdio_config_snippets(
        &runtime_path,
        &orb.slug,
        &orb.zip_path,
        use_wrapper,
    ))
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(to_string)
}

#[tauri::command]
async fn get_settings(state: tauri::State<'_, AppState>) -> Result<RuntimeSettings, String> {
    let settings_store = state.settings.lock().await;
    settings_store.load().map_err(to_string)
}

#[tauri::command]
async fn save_settings(
    settings: RuntimeSettings,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let settings_store = state.settings.lock().await;
    settings_store.save(&settings).map_err(to_string)
}

#[tauri::command]
async fn start_orb_http(
    orb_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<RunningOrb, String> {
    let orb = state
        .registry
        .get(&orb_id)
        .map_err(to_string)?
        .ok_or_else(|| format!("Orb `{orb_id}` is not installed"))?;

    let settings = {
        let settings_store = state.settings.lock().await;
        settings_store.load().map_err(to_string)?
    };

    let runtime_path = default_runtime_binary()
        .ok_or_else(|| "Could not resolve mcporb-runtime binary path".to_string())?;

    let token = token_from_orb_id(&orb_id);
    let port = settings.http_port;

    let mut cmd = tokio::process::Command::new(&runtime_path);
    cmd.arg("--orb-zip")
        .arg(&orb.zip_path)
        .arg("--port")
        .arg(port.to_string())
        .arg("--token")
        .arg(&token)
        .arg("--no-open")
        .arg("--orb-id")
        .arg(&orb_id)
        .arg("--metrics-dir")
        .arg(state.registry.root_dir().join("metrics"));

    if settings.network_binding == mcporb_runtime_app_core::settings::NetworkBinding::External {
        cmd.arg("--bind-external");
    }

    let child = cmd.spawn().map_err(|e| format!("Failed to start Orb: {e}"))?;
    let pid = child.id().unwrap_or(0);

    let running = RunningOrb {
        orb_id: orb_id.clone(),
        slug: orb.slug.clone(),
        port,
        token: token.clone(),
        pid,
    };

    state.running_children.lock().await.insert(orb_id.clone(), child);
    state.running_orbs.lock().await.push(running.clone());

    Ok(running)
}

#[tauri::command]
async fn stop_orb_http(
    orb_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.running_orbs.lock().await.retain(|r| r.orb_id != orb_id);
    if let Some(mut child) = state.running_children.lock().await.remove(&orb_id) {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    Ok(())
}

#[tauri::command]
async fn list_running_orbs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RunningOrb>, String> {
    let running_orbs = state.running_orbs.lock().await;
    Ok(running_orbs.clone())
}

#[tauri::command]
async fn remove_orb(
    orb_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.running_orbs.lock().await.retain(|r| r.orb_id != orb_id);
    if let Some(mut child) = state.running_children.lock().await.remove(&orb_id) {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    state.registry.remove(&orb_id).map_err(to_string)?;
    let metrics_path = state.registry.root_dir().join("metrics").join(format!("{orb_id}.json"));
    if metrics_path.is_file() {
        let _ = std::fs::remove_file(&metrics_path);
    }
    Ok(())
}

#[tauri::command]
async fn get_orb_metrics(
    orb_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<metrics::OrbMetricsSummary, String> {
    // Try file-based metrics first (works for both STDIO and HTTP)
    let metrics_dir = state.registry.root_dir().join("metrics");
    let file_path = metrics_dir.join(format!("{orb_id}.json"));
    if let Some(m) = metrics::read_metrics_from_file(&file_path) {
        return Ok(m);
    }
    // Fall back to HTTP-based metrics
    let running_orbs = state.running_orbs.lock().await;
    if let Some(running) = running_orbs.iter().find(|r| r.orb_id == orb_id) {
        if let Ok(Some(m)) = metrics::fetch_orb_metrics(running.port, &running.token).await {
            return Ok(m);
        }
    }
    Ok(metrics::OrbMetricsSummary::default())
}

#[tauri::command]
async fn get_orb_qa_history(
    orb_id: String,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<metrics::QaHistoryResponse, String> {
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(20);
    // Try file-based metrics first
    let metrics_dir = state.registry.root_dir().join("metrics");
    let file_path = metrics_dir.join(format!("{orb_id}.json"));
    if let Some(history) = metrics::read_qa_history_from_file(&file_path, page, page_size) {
        return Ok(history);
    }
    // Fall back to HTTP
    let running_orbs = state.running_orbs.lock().await;
    if let Some(running) = running_orbs.iter().find(|r| r.orb_id == orb_id) {
        if let Ok(Some(history)) = metrics::fetch_orb_qa_history(
            running.port,
            &running.token,
            page,
            page_size,
        )
        .await
        {
            return Ok(history);
        }
    }
    Ok(metrics::QaHistoryResponse {
        items: vec![],
        page,
        page_size,
        total: 0,
        total_pages: 1,
    })
}

#[tauri::command]
async fn mcp_config_http_snippets(
    orb_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<mcp_config::McpConfigSnippet>, String> {
    let orb = state
        .registry
        .get(&orb_id)
        .map_err(to_string)?
        .ok_or_else(|| format!("Orb `{orb_id}` is not installed"))?;

    let running_orbs = state.running_orbs.lock().await;
    let running = running_orbs
        .iter()
        .find(|r| r.orb_id == orb_id)
        .ok_or_else(|| "Orb is not running. Start it first.".to_string())?;

    Ok(mcp_config::http_config_snippets(
        &orb.slug,
        running.port,
        &running.token,
    ))
}

// ── Platform Config Discovery ──────────────────────────────────────────────

#[tauri::command]
fn discover_platform_configs(state: tauri::State<AppState>) -> Result<Vec<PlatformConfig>, String> {
    let mut configs = platform_config::discover_platform_configs();
    let orbs = state.registry.list().map_err(to_string)?;

    for config in &mut configs {
        let generated = build_merged_platform_config(&config, &orbs);
        config.generated_content = Some(generated);
    }

    Ok(configs)
}

/// Build the merged JSON content for a platform config.
///
/// This injects STDIO MCP entries for all installed orbs into the platform's
/// existing `mcpServers` (if any), preserving entries for other tools.
fn build_merged_platform_config(
    platform: &PlatformConfig,
    orbs: &[InstalledOrb],
) -> String {
    let use_wrapper = is_runner_wrapper_mode();
    let binary = if use_wrapper { mcp_binary_path() } else { default_runtime_binary() };
    let runtime_binary = binary
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "mcporb-runner".to_string());

    let mut server_entries = Vec::new();
    for orb in orbs {
        let key = format!("mcporb-{}", orb.slug);
        let server_value = platform_config::make_stdio_server_config(
            &runtime_binary,
            &orb.id,
            &orb.zip_path.display().to_string(),
            use_wrapper,
        );
        server_entries.push((key, server_value));
    }

    let existing = platform.current_content.as_deref();
    match platform_config::build_merged_config_with_entries(existing, &server_entries) {
        Ok(json) => json,
        Err(_) => {
            let mut map = serde_json::Map::new();
            let mut servers = serde_json::Map::new();
            for (key, value) in &server_entries {
                servers.insert(key.clone(), value.clone());
            }
            map.insert(
                "mcpServers".to_string(),
                serde_json::Value::Object(servers),
            );
            serde_json::to_string_pretty(&serde_json::Value::Object(map))
                .unwrap_or_else(|_| "{}".to_string())
        }
    }
}

#[tauri::command]
fn apply_platform_config(
    config_path: String,
    new_content: String,
    platform: String,
    restart_hint: String,
) -> Result<WriteConfigResult, String> {
    let mut result = platform_config::write_platform_config(&config_path, &new_content)
        .map_err(to_string)?;
    result.platform = platform;
    result.restart_hint = Some(restart_hint);
    Ok(result)
}

#[tauri::command]
fn read_platform_config_raw(config_path: String) -> Result<String, String> {
    platform_config::read_config_raw(&config_path).map_err(to_string)
}

fn token_from_orb_id(orb_id: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(orb_id.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

#[tauri::command]
async fn store_search(
    query: String,
    page: Option<usize>,
    per_page: Option<usize>,
) -> Result<StoreSearchResult, String> {
    let client = StoreClient::new().map_err(to_string)?;
    client
        .search_orbs(&query, page.unwrap_or(1), per_page.unwrap_or(20))
        .await
        .map_err(to_string)
}

#[tauri::command]
async fn store_get_orb(slug: String) -> Result<StoreOrb, String> {
    let client = StoreClient::new().map_err(to_string)?;
    client.get_orb(&slug).await.map_err(to_string)
}

#[tauri::command]
async fn store_download_orb(
    slug: String,
    password: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<ImportResult, String> {
    let settings = {
        let settings_store = state.settings.lock().await;
        settings_store.load().map_err(to_string)?
    };

    let client = StoreClient::new().map_err(to_string)?;
    let orb = client.get_orb(&slug).await.map_err(to_string)?;

    let dest_dir = settings.download_dir.join("store");
    std::fs::create_dir_all(&dest_dir).map_err(to_string)?;

    let dest_path = dest_dir.join(format!("{}.orb.zip", slug));

    if orb.has_password {
        let password = password.ok_or_else(|| "Password required for this Orb".to_string())?;
        let token = client
            .verify_download_password(&orb.slug, &password)
            .await
            .map_err(to_string)?;
        client
            .download_orb(&orb.slug, &token.token, &dest_path)
            .await
            .map_err(to_string)?;
    } else {
        let token = client
            .verify_download_password(&orb.slug, "")
            .await
            .map_err(to_string)?;
        client
            .download_orb(&orb.slug, &token.token, &dest_path)
            .await
            .map_err(to_string)?;
    }

    state
        .registry
        .import_zip(&dest_path, ImportOptions::default())
        .map_err(to_string)
}

fn main() {
    // --- MCP STDIO proxy mode ---
    // Intercept before any Tauri/WebKit init to keep stdio clean and avoid
    // the ~50 MB WebView overhead when launched by an MCP client.
    if std::env::args().any(|a| a == "--mcp-stdio") {
        run_mcp_stdio_proxy();
    }

    tracing_subscriber::fmt::init();

    let registry = RegistryStore::default().unwrap_or_else(|error| {
        tracing::warn!(%error, "falling back to local runtime registry directory");
        RegistryStore::new(PathBuf::from(".mcporb-runtime"))
    });

    let settings = SettingsStore::default().unwrap_or_else(|error| {
        tracing::warn!(%error, "falling back to default settings store");
        SettingsStore::new(PathBuf::from(".mcporb-runtime"))
    });

    let app_state = AppState {
        registry,
        settings: Arc::new(Mutex::new(settings)),
        running_orbs: Arc::new(Mutex::new(Vec::new())),
        running_children: Arc::new(Mutex::new(HashMap::new())),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            runtime_status,
            list_orbs,
            import_orb_zip,
            search_orb,
            mcp_config_snippets,
            open_path,
            get_settings,
            save_settings,
            start_orb_http,
            stop_orb_http,
            list_running_orbs,
            get_orb_metrics,
            get_orb_qa_history,
            mcp_config_http_snippets,
            store_search,
            store_get_orb,
            store_download_orb,
            remove_orb,
            discover_platform_configs,
            apply_platform_config,
            read_platform_config_raw,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCPOrb Runner");
}

/// Run as an MCP STDIO proxy: spawn `mcporb-runtime` with cleaned args and
/// inherit stdin/stdout/stderr so that the MCP client (e.g. Claude Desktop)
/// communicates directly with the runtime while the sandbox container is
/// maintained (the child inherits the sandbox of this sandboxed process).
///
/// Never returns — exits with the child's exit code.
fn run_mcp_stdio_proxy() -> ! {
    let runtime_path = default_runtime_binary().unwrap_or_else(|| {
        eprintln!("mcporb-runner: mcporb-runtime binary not found");
        std::process::exit(1);
    });

    // Resolve the metrics directory so the runtime writes where the GUI reads.
    // Best-effort: if RegistryStore fails, the runtime's own fallback will use
    // the same `dirs::data_dir()` path inside the sandbox container.
    let metrics_dir = RegistryStore::default()
        .ok()
        .map(|r| r.root_dir().join("metrics"));

    let mut runtime_args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--mcp-stdio")
        .collect();

    // The runtime needs --stdio-only for MCP protocol mode
    if !runtime_args.iter().any(|a| a == "--stdio-only") {
        runtime_args.push("--stdio-only".to_string());
    }
    // Ensure metrics land where get_orb_metrics reads them
    if let Some(ref dir) = metrics_dir {
        if !runtime_args.iter().any(|a| a == "--metrics-dir") {
            runtime_args.push("--metrics-dir".to_string());
            runtime_args.push(dir.display().to_string());
        }
    }

    let mut child = match std::process::Command::new(&runtime_path)
        .args(&runtime_args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mcporb-runner: failed to spawn mcporb-runtime: {e}");
            std::process::exit(1);
        }
    };

    let status = child.wait().unwrap_or_else(|e| {
        eprintln!("mcporb-runner: failed to wait for mcporb-runtime: {e}");
        std::process::exit(1);
    });
    std::process::exit(status.code().unwrap_or(1));
}

/// Whether the generated MCP client config should point to `mcporb-runner`
/// (the sandboxed wrapper) instead of directly to `mcporb-runtime`.
///
/// Returns `true` when `mcp_binary_path()` resolves to the runner binary
/// (i.e. `mcporb-runner` lives next to itself), which is the case in MAS
/// builds. In that scenario the MCP client launches the sandboxed Runner,
/// which in turn spawns the Runtime as a child process.
fn is_runner_wrapper_mode() -> bool {
    mcp_binary_path()
        .and_then(|p| p.file_stem().map(|s| s == "mcporb-runner"))
        .unwrap_or(false)
}

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

/// The binary path to use in MCP client config (STDIO command).
/// In MAS builds this is mcporb-runner (the wrapper) so that Claude Desktop
/// spawns the sandboxed Tauri app, which in turn spawns mcporb-runtime as a
/// child process inheriting the sandbox container.
fn mcp_binary_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let suffix = std::env::consts::EXE_SUFFIX;
    let runner = dir.join(format!("mcporb-runner{suffix}"));
    if runner.is_file() {
        Some(runner)
    } else {
        default_runtime_binary()
    }
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}
