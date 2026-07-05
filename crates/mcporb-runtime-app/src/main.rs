#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use mcporb_runtime_app_core::{
    mcp_config, metrics, platform_config, search, settings::SettingsStore, store_client::StoreClient,
    ImportOptions, ImportResult, InstalledOrb, PlatformConfig, RegistryStore, RuntimeSettings,
    SearchResponse, StoreOrb, StoreSearchResult, WriteConfigResult,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    registry: RegistryStore,
    settings: Arc<Mutex<SettingsStore>>,
    running_orbs: Arc<Mutex<Vec<RunningOrb>>>,
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
    registry_dir: String,
    store_status: &'static str,
    http_mcp_status: &'static str,
}

#[tauri::command]
fn runtime_status(state: tauri::State<AppState>) -> Result<RuntimeStatus, String> {
    Ok(RuntimeStatus {
        version: env!("CARGO_PKG_VERSION"),
        registry_dir: state.registry.root_dir().display().to_string(),
        store_status: "planned_mvp_2",
        http_mcp_status: "planned_mvp_4_pending_mas_network_server_entitlement",
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
    let runtime_path = runtime_binary
        .map(PathBuf::from)
        .or_else(default_runtime_binary)
        .ok_or_else(|| "Could not resolve mcporb-runtime binary path".to_string())?;
    Ok(mcp_config::stdio_config_snippets(
        &runtime_path,
        &orb.slug,
        &orb.zip_path,
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

    let mut running_orbs = state.running_orbs.lock().await;
    running_orbs.push(running.clone());

    Ok(running)
}

#[tauri::command]
async fn stop_orb_http(
    orb_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut running_orbs = state.running_orbs.lock().await;
    if let Some(pos) = running_orbs.iter().position(|r| r.orb_id == orb_id) {
        let running = running_orbs.remove(pos);
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(running.pid as i32),
            nix::sys::signal::Signal::SIGTERM,
        );
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
    {
        let mut running_orbs = state.running_orbs.lock().await;
        if let Some(pos) = running_orbs.iter().position(|r| r.orb_id == orb_id) {
            let running = running_orbs.remove(pos);
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(running.pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
        }
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
    let runtime_binary = default_runtime_binary()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "mcporb-runtime".to_string());

    let mut server_entries = Vec::new();
    for orb in orbs {
        let key = format!("mcporb-{}", orb.slug);
        let server_value = platform_config::make_stdio_server_config(
            &runtime_binary,
            &orb.id,
            &orb.zip_path.display().to_string(),
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
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .manage(app_state)
        .setup(|app| {
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_deep_link(&handle, url.as_str());
                }
            });
            Ok(())
        })
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
        .expect("error while running MCPOrb Runtime App");
}

fn handle_deep_link(handle: &tauri::AppHandle, url: &str) {
    tracing::info!(url, "received Runtime deep link");

    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    match parse_deep_link(url) {
        Ok(DeepLinkAction::ImportZip { path }) => {
            let _ = handle.emit("runtime:deep-link-import", path);
        }
        Ok(DeepLinkAction::InstallFromStore { slug, version }) => {
            let _ = handle.emit(
                "runtime:deep-link-install",
                serde_json::json!({ "slug": slug, "version": version }),
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse deep link URL");
        }
    }
}

enum DeepLinkAction {
    ImportZip { path: String },
    InstallFromStore { slug: String, version: Option<String> },
}

fn parse_deep_link(url: &str) -> Result<DeepLinkAction, String> {
    let url = url
        .strip_prefix("mcporb://")
        .ok_or_else(|| format!("not a mcporb:// URL: {url}"))?;

    let (action, query) = url
        .split_once('?')
        .unwrap_or((url, ""));

    let params: std::collections::HashMap<String, String> = query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect();

    match action {
        "import" => {
            let path = params
                .get("path")
                .cloned()
                .ok_or_else(|| "missing 'path' parameter".to_string())?;
            Ok(DeepLinkAction::ImportZip { path })
        }
        "install" => {
            let slug = params
                .get("slug")
                .cloned()
                .ok_or_else(|| "missing 'slug' parameter".to_string())?;
            let version = params.get("version").cloned();
            Ok(DeepLinkAction::InstallFromStore { slug, version })
        }
        _ => Err(format!("unknown deep link action: {action}")),
    }
}

fn default_runtime_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let suffix = std::env::consts::EXE_SUFFIX;
    for name in ["mcporb-runtime-lite", "mcporb-runtime"] {
        let path = dir.join(format!("{name}{suffix}"));
        if path.is_file() {
            return Some(path);
        }
    }
    Some(dir.join(format!("mcporb-runtime{suffix}")))
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}
