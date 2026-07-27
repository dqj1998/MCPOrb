#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use mcporb_runtime_app_core::{
    mcp_config, metrics, platform_config, search, settings::SettingsStore, store_client::StoreClient,
    validate_zip_path, ImportOptions, InstalledOrb, PlatformConfig, RegistryStore, RuntimeSettings,
    DownloadToken, ListResponse, OrbDetail, SearchResponse, TagInfo, WriteConfigResult,
    inspect_orb_security, remember_orb_password as remember_orb_password_impl,
    verify_orb_password as verify_orb_password_impl, OrbSecurityInfo,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::Mutex;

const ORB_UNLOCK_PASSWORD_ENV: &str = "MCPORB_UNLOCK_PASSWORD";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrbStartRequest {
    orb_id: String,
    password: Option<String>,
}

#[derive(Debug, Serialize)]
struct ZipInspectResult {
    password_protected: bool,
    manifest_name: String,
    manifest_version: String,
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

#[derive(Debug, Serialize)]
struct ImportWithSecurity {
    report: mcporb_runtime_app_core::ZipValidationReport,
    stored_zip_path: PathBuf,
    security: Option<OrbSecurityInfo>,
}

#[tauri::command]
fn inspect_zip(path: String) -> Result<ZipInspectResult, String> {
    let report = validate_zip_path(&PathBuf::from(&path)).map_err(to_string)?;
    Ok(ZipInspectResult {
        password_protected: report.password_protected,
        manifest_name: report.manifest.display_name.unwrap_or(report.manifest.name),
        manifest_version: report.manifest.version,
    })
}

#[tauri::command]
fn import_orb_zip(path: String, password: Option<String>, state: tauri::State<AppState>) -> Result<ImportWithSecurity, String> {
    let src = PathBuf::from(&path);

    let pre_check = validate_zip_path(&src).map_err(to_string)?;
    if pre_check.password_protected {
        let pwd = password.as_deref().unwrap_or("");
        if pwd.is_empty() {
            return Err("This Orb is password-protected. Please provide the password to import it.".to_string());
        }
        if !verify_orb_password_impl(&src, pwd).map_err(to_string)? {
            return Err("Incorrect password for this Orb.".to_string());
        }
    }

    let result = state
        .registry
        .import_zip(&src, ImportOptions::default())
        .map_err(to_string)?;
    let security = inspect_orb_security(&result.stored_zip_path).ok();

    if let Some(ref pwd) = password {
        if pre_check.password_protected && !pwd.is_empty() {
            if let Err(e) = remember_orb_password_impl(&result.stored_zip_path, pwd) {
                eprintln!("[MCPOrb] warning: failed to remember password in keychain: {e}");
            }
        }
    }

    Ok(ImportWithSecurity {
        report: result.report,
        stored_zip_path: result.stored_zip_path,
        security,
    })
}

#[tauri::command]
fn search_orb(
    orb_id: String,
    query: String,
    password: Option<String>,
    method: Option<String>,
    top_k: Option<usize>,
    state: tauri::State<AppState>,
) -> Result<SearchResponse, String> {
    let orb = state
        .registry
        .get(&orb_id)
        .map_err(to_string)?
        .ok_or_else(|| format!("Orb `{orb_id}` is not installed"))?;
    if let Some(ref pwd) = password {
        search::search_zip_with_password(&orb.zip_path, &query, method.as_deref(), top_k, pwd)
            .map_err(to_string)
    } else {
        search::search_zip(&orb.zip_path, &query, method.as_deref(), top_k).map_err(to_string)
    }
}

#[tauri::command]
fn gateway_mcp_config_snippets(
    state: tauri::State<AppState>,
) -> Result<Vec<mcp_config::McpConfigSnippet>, String> {
    let gateway_bin = default_gateway_binary()
        .ok_or_else(|| "Could not resolve mcporb-gateway-stdio binary path".to_string())?;
    let registry_dir = state.registry.root_dir().to_path_buf();
    // Keep the old per-orb command so existing callers don't break
    Ok(mcp_config::gateway_stdio_config_snippets(&gateway_bin, &registry_dir))
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
        Some(&orb_id),
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
async fn get_orb_security(
    orb_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<OrbSecurityInfo, String> {
    let orb = state
        .registry
        .get(&orb_id)
        .map_err(to_string)?
        .ok_or_else(|| format!("Orb `{orb_id}` is not installed"))?;
    inspect_orb_security(&orb.zip_path).map_err(to_string)
}

#[tauri::command]
async fn start_orb_http(
    orb_id: String,
    password: Option<String>,
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

    if let Some(password) = password.filter(|value| !value.is_empty()) {
        cmd.env(ORB_UNLOCK_PASSWORD_ENV, OsString::from(password));
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
async fn batch_start_orbs(
    orbs: Vec<OrbStartRequest>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RunningOrb>, String> {
    let settings = {
        let settings_store = state.settings.lock().await;
        settings_store.load().map_err(to_string)?
    };

    let runtime_path = default_runtime_binary()
        .ok_or_else(|| "Could not resolve mcporb-runtime binary path".to_string())?;
    let token_base = |orb_id: &str| -> String {
        use base64::Engine;
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(orb_id.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
    };

    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut children_lock = state.running_children.lock().await;
    let mut running_lock = state.running_orbs.lock().await;

    for req in orbs {
            if children_lock.contains_key(&req.orb_id) {
            if let Some(r) = running_lock.iter().find(|r| r.orb_id == req.orb_id) {
                results.push(r.clone());
            }
            continue;
        }

        let orb = match state.registry.get(&req.orb_id) {
            Ok(Some(orb)) => orb,
            Ok(None) => {
                errors.push(format!("{}: not installed", req.orb_id));
                continue;
            }
            Err(e) => {
                errors.push(format!("{}: {e}", req.orb_id));
                continue;
            }
        };

        let token = token_base(&req.orb_id);
        let port = settings.http_port + results.len() as u16;

        let mut cmd = tokio::process::Command::new(&runtime_path);
        cmd.arg("--orb-zip")
            .arg(&orb.zip_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--token")
            .arg(&token)
            .arg("--no-open")
            .arg("--orb-id")
            .arg(&req.orb_id)
            .arg("--metrics-dir")
            .arg(state.registry.root_dir().join("metrics"));

        if settings.network_binding == mcporb_runtime_app_core::settings::NetworkBinding::External {
            cmd.arg("--bind-external");
        }

        if let Some(password) = req.password.filter(|value| !value.is_empty()) {
            cmd.env(ORB_UNLOCK_PASSWORD_ENV, std::ffi::OsString::from(password));
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id().unwrap_or(0);
                let running = RunningOrb {
                    orb_id: req.orb_id.clone(),
                    slug: orb.slug.clone(),
                    port,
                    token: token.clone(),
                    pid,
                };
                children_lock.insert(req.orb_id.clone(), child);
                running_lock.push(running.clone());
                results.push(running);
            }
            Err(e) => {
                errors.push(format!("{}: failed to spawn: {e}", req.orb_id));
            }
        }
    }

    drop(children_lock);
    drop(running_lock);

    if !errors.is_empty() {
        tracing::warn!("batch_start_orbs partial errors: {:?}", errors);
    }

    Ok(results)
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

async fn shutdown_running_orbs(state: &AppState) {
    state.running_orbs.lock().await.clear();

    let mut children = state.running_children.lock().await;
    let mut drained: Vec<(String, tokio::process::Child)> = children.drain().collect();
    drop(children);

    for (_, child) in drained.iter_mut() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

#[tauri::command]
async fn gateway_http_config_snippets(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<mcp_config::McpConfigSnippet>, String> {
    let settings = {
        let settings_store = state.settings.lock().await;
        settings_store.load().map_err(to_string)?
    };
    Ok(mcp_config::gateway_http_config_snippets(settings.http_port))
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
/// Injects a single `mcporb-gateway` STDIO entry — the gateway routes to all
/// installed Orbs via namespace prefix — into the platform's existing
/// `mcpServers` (if any), preserving entries for other tools.
fn build_merged_platform_config(
    platform: &PlatformConfig,
    orbs: &[InstalledOrb],
) -> String {
    let registry_dir = PathBuf::from(
        // Best-effort: use the same default as RegistryStore::default().
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("MCPOrb")
            .join("Runtime"),
    );

    // Use gateway binary if available, fall back to per-orb for empty registries

    let use_gateway = default_gateway_binary().is_some() && !orbs.is_empty();

    if use_gateway {
        let gateway_bin = default_gateway_binary().unwrap();
        let server_value = platform_config::make_stdio_server_config(
            &gateway_bin.display().to_string(),
            "gateway",
            &registry_dir.display().to_string(),
            false,
        );
        let entry = ("mcporb-gateway".to_string(), server_value);
        let existing = platform.current_content.as_deref();
        return match platform_config::build_merged_config_with_entries(existing, &[entry]) {
            Ok(json) => json,
            Err(_) => {
                let mut map = serde_json::Map::new();
                let mut servers = serde_json::Map::new();
                servers.insert("mcporb-gateway".to_string(), serde_json::json!({
                    "command": gateway_bin.display().to_string(),
                    "args": ["--registry-dir", registry_dir.display().to_string()]
                }));
                map.insert("mcpServers".to_string(), serde_json::Value::Object(servers));
                serde_json::to_string_pretty(&serde_json::Value::Object(map))
                    .unwrap_or_else(|_| "{}".to_string())
            }
        };
    }

    // Fallback: per-orb entries (no gateway binary found)
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
            map.insert("mcpServers".to_string(), serde_json::Value::Object(servers));
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
    tag: Option<String>,
    method: Option<String>,
    page: Option<i64>,
) -> Result<ListResponse, String> {
    let client = StoreClient::new().map_err(to_string)?;
    client
        .search_orbs_filtered(&query, tag.as_deref(), method.as_deref(), page.unwrap_or(1))
        .await
        .map_err(to_string)
}

#[tauri::command]
async fn store_get_orb(slug: String) -> Result<OrbDetail, String> {
    let client = StoreClient::new().map_err(to_string)?;
    client.get_orb(&slug).await.map_err(to_string)
}

#[tauri::command]
async fn store_list_tags() -> Result<Vec<TagInfo>, String> {
    let client = StoreClient::new().map_err(to_string)?;
    client.list_tags().await.map_err(to_string)
}

#[tauri::command]
async fn store_verify_download_password(
    artifact_id: String,
    password: String,
) -> Result<DownloadToken, String> {
    let client = StoreClient::new().map_err(to_string)?;
    client
        .verify_download_password(&artifact_id, &password)
        .await
        .map_err(to_string)
}

#[tauri::command]
async fn store_download_artifact(
    artifact_id: String,
    token: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let settings = {
        let settings_store = state.settings.lock().await;
        settings_store.load().map_err(to_string)?
    };

    let client = StoreClient::new().map_err(to_string)?;

    let dest_dir = settings.download_dir.join("store");
    std::fs::create_dir_all(&dest_dir).map_err(to_string)?;
    let dest_path = dest_dir.join(format!("{artifact_id}.zip"));
    let token = token.unwrap_or_default();
    let _size = client
        .download_orb(&artifact_id, &token, &dest_path)
        .await
        .map_err(to_string)?;

    Ok(dest_path.to_string_lossy().to_string())
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
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.app_handle().state::<AppState>().inner().clone();
                tauri::async_runtime::block_on(async move {
                    shutdown_running_orbs(&state).await;
                });
            }
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            runtime_status,
            list_orbs,
            inspect_zip,
            import_orb_zip,
            search_orb,
            get_orb_security,
            gateway_mcp_config_snippets,
            mcp_config_snippets,
            open_path,
            get_settings,
            save_settings,
            start_orb_http,
            stop_orb_http,
            list_running_orbs,
            get_orb_metrics,
            get_orb_qa_history,
            gateway_http_config_snippets,
            mcp_config_http_snippets,
            store_search,
            store_get_orb,
            store_list_tags,
            store_verify_download_password,
            store_download_artifact,
            remove_orb,
            discover_platform_configs,
            apply_platform_config,
            read_platform_config_raw,
            batch_start_orbs,
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

/// Look for `mcporb-runtime` (or `mcporb-runtime-lite`) in `dir`.
/// Returns `Some(dir / "mcporb-runtime{EXE_SUFFIX}")` even if the file does not
/// exist (caller should handle the missing case at spawn time), matching the
/// original behaviour of returning a default path as a last resort.
fn resolve_runtime_binary_in(dir: &std::path::Path) -> Option<PathBuf> {
    let suffix = std::env::consts::EXE_SUFFIX;
    for name in ["mcporb-runtime", "mcporb-runtime-lite"] {
        let path = dir.join(format!("{name}{suffix}"));
        if path.is_file() {
            return Some(path);
        }
    }
    Some(dir.join(format!("mcporb-runtime{suffix}")))
}

fn default_runtime_binary() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    resolve_runtime_binary_in(&dir)
}

/// The binary path to use in MCP client config (STDIO command).
/// In MAS builds this is mcporb-runner (the wrapper) so that Claude Desktop
/// spawns the sandboxed Tauri app, which in turn spawns mcporb-runtime as a
/// child process inheriting the sandbox container.
fn resolve_mcp_binary_in(dir: &std::path::Path) -> Option<PathBuf> {
    let suffix = std::env::consts::EXE_SUFFIX;
    let runner = dir.join(format!("mcporb-runner{suffix}"));
    if runner.is_file() {
        Some(runner)
    } else {
        resolve_runtime_binary_in(dir)
    }
}

fn mcp_binary_path() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    resolve_mcp_binary_in(&dir)
}

fn resolve_gateway_binary_in(dir: &std::path::Path) -> Option<PathBuf> {
    let suffix = std::env::consts::EXE_SUFFIX;
    let path = dir.join(format!("mcporb-gateway-stdio{suffix}"));
    if path.is_file() { Some(path) } else { None }
}

/// The path to the `mcporb-gateway-stdio` binary, resolved relative to the
/// current executable.
fn default_gateway_binary() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    resolve_gateway_binary_in(&dir)
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    // ── binary resolution tests ──────────────────────────────────────────────

    #[test]
    fn resolve_gateway_binary_returns_none_in_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve_gateway_binary_in(dir.path()), None);
    }

    #[test]
    fn resolve_gateway_binary_finds_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let bin = dir.path().join(format!("mcporb-gateway-stdio{suffix}"));
        fs::write(&bin, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolve_gateway_binary_in(dir.path()), Some(bin));
    }

    #[test]
    fn resolve_gateway_binary_ignores_wrong_name() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let bin = dir.path().join(format!("mcporb-runtime{suffix}"));
        fs::write(&bin, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        // gateway binary is absent, should not find runtime
        assert_eq!(resolve_gateway_binary_in(dir.path()), None);
    }

    #[test]
    fn resolve_runtime_binary_finds_exact_name() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let bin = dir.path().join(format!("mcporb-runtime{suffix}"));
        fs::write(&bin, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolve_runtime_binary_in(dir.path()), Some(bin));
    }

    #[test]
    fn resolve_runtime_binary_prefers_exact_over_lite() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let full = dir.path().join(format!("mcporb-runtime{suffix}"));
        let lite = dir.path().join(format!("mcporb-runtime-lite{suffix}"));
        fs::write(&full, "").unwrap();
        fs::write(&lite, "").unwrap();
        #[cfg(unix)]
        {
            fs::set_permissions(&full, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&lite, fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(resolve_runtime_binary_in(dir.path()), Some(full));
    }

    #[test]
    fn resolve_runtime_binary_falls_back_to_default_path() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        // No file exists, but the function returns a default path anyway
        let expected = dir.path().join(format!("mcporb-runtime{suffix}"));
        assert_eq!(resolve_runtime_binary_in(dir.path()), Some(expected));
    }

    #[test]
    fn resolve_mcp_binary_prefers_runner_over_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let runner = dir.path().join(format!("mcporb-runner{suffix}"));
        let runtime = dir.path().join(format!("mcporb-runtime{suffix}"));
        fs::write(&runner, "").unwrap();
        fs::write(&runtime, "").unwrap();
        #[cfg(unix)]
        {
            fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&runtime, fs::Permissions::from_mode(0o755)).unwrap();
        }
        // runner takes precedence
        assert_eq!(resolve_mcp_binary_in(dir.path()), Some(runner));
    }

    #[test]
    fn resolve_mcp_binary_falls_back_to_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let runtime = dir.path().join(format!("mcporb-runtime{suffix}"));
        fs::write(&runtime, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o755)).unwrap();
        // no runner, should find runtime
        assert_eq!(resolve_mcp_binary_in(dir.path()), Some(runtime));
    }

    // ── existing tests ───────────────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn shutdown_running_orbs_clears_state_and_kills_children() {
        tauri::async_runtime::block_on(async {
            let child = tokio::process::Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("spawn sleep child");
            let pid = child.id().unwrap_or(0);

            let app_state = AppState {
                registry: RegistryStore::new(PathBuf::from(".mcporb-runtime-test")),
                settings: Arc::new(Mutex::new(SettingsStore::new(PathBuf::from(
                    ".mcporb-runtime-test",
                )))),
                running_orbs: Arc::new(Mutex::new(vec![RunningOrb {
                    orb_id: "orb-1".to_string(),
                    slug: "orb-1".to_string(),
                    port: 7777,
                    token: "token".to_string(),
                    pid,
                }])),
                running_children: Arc::new(Mutex::new(HashMap::new())),
            };

            app_state
                .running_children
                .lock()
                .await
                .insert("orb-1".to_string(), child);

            shutdown_running_orbs(&app_state).await;

            assert!(app_state.running_orbs.lock().await.is_empty());
            assert!(app_state.running_children.lock().await.is_empty());
        });
    }
}
