#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use mcporb_runtime_app_core::{
    mcp_config, metrics, platform_config, search, settings::SettingsStore, settings::NetworkBinding,
    store_client::StoreClient,
    validate_zip_path, ImportOptions, InstalledOrb, PlatformConfig, RegistryStore, RuntimeSettings,
    DownloadToken, ListResponse, OrbDetail, SearchResponse, TagInfo, WriteConfigResult,
    inspect_orb_security, remember_orb_password as remember_orb_password_impl,
    verify_orb_password as verify_orb_password_impl, OrbSecurityInfo,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::Mutex;

const ORB_UNLOCK_PASSWORD_ENV: &str = "MCPORB_UNLOCK_PASSWORD";

#[cfg(target_os = "macos")]
mod macos_access;
#[cfg(target_os = "macos")]
mod macos_panel;

#[derive(Clone)]
struct AppState {
    registry: RegistryStore,
    settings: Arc<Mutex<SettingsStore>>,
    running_orbs: Arc<Mutex<Vec<RunningOrb>>>,
    running_children: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
    /// Picked-but-unapplied library folder (macOS); consumed by
    /// `apply_orb_library_change` / `cancel_orb_library_change`.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pending_library_pick: Arc<Mutex<Option<PendingLibraryPick>>>,
    /// Security-scoped access to the active library folder (macOS sandbox);
    /// swapped for the new folder's guard after a migration.
    #[cfg(target_os = "macos")]
    library_access: Arc<Mutex<Option<macos_access::AccessGuard>>>,
    /// Set when the stored Orb library bookmark is stale (e.g. after a
    /// TestFlight reinstall). The UI checks this to prompt re-selection.
    #[cfg(target_os = "macos")]
    library_bookmark_stale: Arc<AtomicBool>,
}

/// Folder picked in the dialog plus its security-scoped bookmark, held until
/// the user decides what to do with the orbs in the old folder.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone)]
struct PendingLibraryPick {
    path: PathBuf,
    bookmark: String,
}

/// Result of picking / applying an Orb library folder change.
#[derive(Debug, Serialize)]
struct OrbLibraryPick {
    path: String,
    /// true = settings not saved yet; frontend must ask migrate/delete for
    /// the `orb_count` orbs in the old folder before anything is applied.
    pending: bool,
    orb_count: usize,
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

/// Reserved key for the unified HTTP gateway child process in
/// `running_children`. It reuses the same lifecycle map as per-orb servers, so
/// `shutdown_running_orbs` already tears it down on window close — no extra
/// shutdown wiring needed.
const UNIFIED_GATEWAY_KEY: &str = "__unified_gateway__";

/// Status of the unified HTTP gateway, serialized for the frontend.
#[derive(Debug, Serialize)]
struct UnifiedGatewayStatus {
    running: bool,
    port: u16,
    url: String,
    token: Option<String>,
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
fn get_platform() -> &'static str {
    std::env::consts::OS
}

#[derive(Debug, Serialize)]
struct LibraryHealth {
    /// true when the saved Orb library bookmark is stale or inaccessible,
    /// meaning the user must re-select the folder via Settings → Choose…
    bookmark_stale: bool,
}

/// Returns macOS Orb library bookmark health. Called by the Settings UI on
/// load so it can show a re-select warning after e.g. a TestFlight reinstall.
#[tauri::command]
#[cfg(target_os = "macos")]
fn get_library_health(state: tauri::State<AppState>) -> LibraryHealth {
    LibraryHealth {
        bookmark_stale: state.library_bookmark_stale.load(Ordering::Relaxed),
    }
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
fn get_library_health(_state: tauri::State<AppState>) -> LibraryHealth {
    LibraryHealth { bookmark_stale: false }
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

    #[cfg(target_os = "macos")]
    {
        let runner_bin = gateway_bin
            .parent()
            .and_then(resolve_runner_binary_in)
            .or_else(mcp_binary_path)
            .ok_or_else(|| "Could not resolve mcporb-runner binary path".to_string())?;

        let value = serde_json::json!({
            "mcpServers": {
                "mcporb-gateway": {
                    "command": runner_bin.display().to_string(),
                    "args": [
                        "--gateway-stdio",
                        "--registry-dir",
                        registry_dir.display().to_string(),
                    ]
                }
            }
        });
        let json = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());

        return Ok([
            ("claude_desktop", "Claude Desktop"),
            ("cursor", "Cursor"),
            ("vscode", "VS Code"),
        ]
        .into_iter()
        .map(|(client, label)| mcp_config::McpConfigSnippet {
            client: client.to_string(),
            label: label.to_string(),
            json: json.clone(),
        })
        .collect());
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(mcp_config::gateway_stdio_config_snippets(&gateway_bin, &registry_dir))
    }
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
    let binding_changed = {
        let store = state.settings.lock().await;
        let current = store.load().unwrap_or_default();
        let old_binding = current.network_binding.clone();
        let merged = merge_settings(current, settings);
        let binding_changed = merged.network_binding != old_binding;
        store.save(&merged).map_err(to_string)?;
        binding_changed
    };
    // The gateway bind address is a spawn-time argument; a binding change
    // only takes effect after a restart, so bounce the process in-place.
    if binding_changed {
        stop_unified_gateway_impl(&state).await?;
        ensure_unified_gateway_impl(&state).await?;
    }
    Ok(())
}

/// Merge frontend-supplied settings over the stored ones. The frontend form
/// never edits the library bookmark, the gateway token, or the onboarding
/// flag; preserving them keeps the "configure the MCP client once" contract —
/// dropping the token here would silently rotate it on any unrelated save.
fn merge_settings(current: RuntimeSettings, mut incoming: RuntimeSettings) -> RuntimeSettings {
    if incoming.orb_library_dir.is_none() {
        incoming.orb_library_dir = current.orb_library_dir;
        incoming.orb_library_bookmark = current.orb_library_bookmark;
    } else if incoming.orb_library_bookmark.is_none()
        && current.orb_library_dir.as_deref() == incoming.orb_library_dir.as_deref()
    {
        incoming.orb_library_bookmark = current.orb_library_bookmark;
    }
    if incoming.gateway_token.as_deref().map_or(true, str::is_empty) {
        incoming.gateway_token = current.gateway_token;
    }
    incoming
}

#[cfg(target_os = "macos")]
async fn pick_orb_library_impl(
    app: &tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    suggested: Option<PathBuf>,
) -> Result<OrbLibraryPick, String> {
    // Run NSOpenPanel directly instead of going through the dialog plugin:
    // the plugin (via rfd) strips the panel result down to a PathBuf, losing
    // the security-scoped NSURL that the bookmark must be created from.
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        // Safety: run_on_main_thread dispatches to the main thread.
        let result = unsafe { macos_panel::pick_library_folder(suggested) };
        let _ = tx.send(result);
    })
    .map_err(|e| format!("Could not show folder picker: {e}"))?;
    let picked = rx
        .await
        .map_err(|_| "Folder dialog closed unexpectedly.".to_string())??;
    let Some(picked) = picked else {
        return Ok(OrbLibraryPick {
            path: String::new(),
            pending: false,
            orb_count: 0,
        });
    };

    let store = state.settings.lock().await;
    let mut settings = store.load().map_err(to_string)?;

    // Orbs outside the newly picked folder would be left behind: they live in
    // the current library folder or in the legacy default app-data Orbs dir
    // (imported before any library folder was configured). Keep the pick
    // pending so the frontend can ask the user to migrate or delete them.
    let same_dir = match settings.orb_library_dir.as_deref() {
        Some(old) => old == picked.path
            || matches!(
                (std::fs::canonicalize(old), std::fs::canonicalize(&picked.path)),
                (Ok(a), Ok(b)) if a == b
            ),
        None => false,
    };
    if !same_dir {
        let new_orbs_dir = picked.path.join("Orbs");
        let mut old_orbs_dirs: Vec<PathBuf> = Vec::new();
        if let Some(old) = settings.orb_library_dir.as_deref() {
            old_orbs_dirs.push(old.join("Orbs"));
        }
        let legacy_orbs_dir = default_registry_root().join("Orbs");
        if legacy_orbs_dir != new_orbs_dir && !old_orbs_dirs.contains(&legacy_orbs_dir) {
            old_orbs_dirs.push(legacy_orbs_dir);
        }
        let count = state
            .registry
            .list()
            .map_err(to_string)?
            .iter()
            .filter(|orb| {
                old_orbs_dirs.iter().any(|dir| orb.zip_path.starts_with(dir))
                    && !orb.zip_path.starts_with(&new_orbs_dir)
            })
            .count();
        if count > 0 {
            state.pending_library_pick.lock().await.replace(PendingLibraryPick {
                path: picked.path.clone(),
                bookmark: picked.bookmark,
            });
            return Ok(OrbLibraryPick {
                path: picked.path.to_string_lossy().into_owned(),
                pending: true,
                orb_count: count,
            });
        }
    }

    settings.orb_library_dir = Some(picked.path.clone());
    settings.orb_library_bookmark = Some(picked.bookmark);
    settings.onboarding_complete = true;
    store.save(&settings).map_err(to_string)?;

    // Same-session consistency, mirroring apply_orb_library_change: repoint
    // imports at the new folder and keep it sandbox-accessible now.
    state.registry.set_orbs_dir(picked.path.join("Orbs"));
    state.library_access.lock().await.replace(picked.guard);
    state.library_bookmark_stale.store(false, Ordering::Relaxed);

    Ok(OrbLibraryPick {
        path: picked.path.to_string_lossy().into_owned(),
        pending: false,
        orb_count: 0,
    })
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn choose_orb_library_dir(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<OrbLibraryPick, String> {
    pick_orb_library_impl(&app, state, None).await
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn choose_orb_library_dir() -> Result<OrbLibraryPick, String> {
    Err("Orb library folder selection is only available on macOS.".to_string())
}

/// Opens the folder picker pre-navigated to the previously stored Orb library
/// folder when one exists (stale-bookmark recovery is then a one-click re-select
/// of the same folder), falling back to ~/Documents/MCPOrb for first-run
/// onboarding. On success marks onboarding done.
#[cfg(target_os = "macos")]
#[tauri::command]
async fn choose_orb_library_dir_suggested(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<OrbLibraryPick, String> {
    let suggested = state
        .settings
        .lock()
        .await
        .load()
        .ok()
        .and_then(|s| s.orb_library_dir)
        .filter(|p| p.is_dir())
        .or_else(|| {
            dirs::document_dir()
                .map(|d| d.join("MCPOrb"))
                .or_else(dirs::home_dir)
        });
    pick_orb_library_impl(&app, state, suggested).await
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn choose_orb_library_dir_suggested() -> Result<OrbLibraryPick, String> {
    Err("Orb library folder selection is only available on macOS.".to_string())
}

/// Applies a pending library-folder change: `action` is "migrate" (move the
/// old folder's Orb ZIPs to the new folder and keep the orbs) or "delete"
/// (remove the old folder's orbs). On success saves the new folder in the
/// settings and repoints the registry so same-session imports land there.
#[cfg(target_os = "macos")]
#[tauri::command]
async fn apply_orb_library_change(
    action: String,
    state: tauri::State<'_, AppState>,
) -> Result<OrbLibraryPick, String> {
    let pending = state
        .pending_library_pick
        .lock()
        .await
        .clone()
        .ok_or_else(|| "No pending Orb library change. Choose a folder first.".to_string())?;

    let settings_store = state.settings.lock().await;
    let settings = settings_store.load().map_err(to_string)?;

    // Resolve both bookmarks before consuming the pending pick so a failure
    // leaves it intact for retry or cancel. The old bookmark is optional: it
    // covers orbs in the previous library folder, while orbs imported before
    // any library folder existed live in the legacy app-data Orbs dir, which
    // needs no bookmark.
    let (new_path, new_guard) =
        macos_access::resolve_bookmark(&pending.bookmark).map_err(to_string)?;
    let new_orbs_dir = new_path.join("Orbs");

    let mut old_orbs_dirs: Vec<PathBuf> = Vec::new();
    let mut _old_guard: Option<macos_access::AccessGuard> = None;
    if let Some(old_bookmark) = settings.orb_library_bookmark.as_deref() {
        match macos_access::resolve_bookmark(old_bookmark) {
            Ok((old_path, guard)) => {
                old_orbs_dirs.push(old_path.join("Orbs"));
                _old_guard = Some(guard);
            }
            Err(error) => {
                // Bookmarks saved before the panel-URL fix carry no security
                // scope and cannot be resolved; keep going so the change can
                // still be applied for the legacy-dir orbs.
                tracing::warn!(
                    %error,
                    "failed to resolve old Orb library bookmark; skipping old library folder"
                );
            }
        }
    }
    let legacy_orbs_dir = default_registry_root().join("Orbs");
    if legacy_orbs_dir != new_orbs_dir && !old_orbs_dirs.contains(&legacy_orbs_dir) {
        old_orbs_dirs.push(legacy_orbs_dir);
    }

    let orb_count = match action.as_str() {
        "migrate" => state
            .registry
            .migrate_orbs(&old_orbs_dirs, &new_orbs_dir)
            .map_err(to_string)?,
        "delete" => state
            .registry
            .delete_orbs(&old_orbs_dirs)
            .map_err(to_string)?,
        other => return Err(format!("Unknown action `{other}`")),
    };

    state.pending_library_pick.lock().await.take();
    let mut settings = settings_store.load().map_err(to_string)?;
    settings.orb_library_dir = Some(pending.path.clone());
    settings.orb_library_bookmark = Some(pending.bookmark);
    settings.onboarding_complete = true;
    settings_store.save(&settings).map_err(to_string)?;

    // Keep same-session imports pointing at the new folder and keep it
    // accessible under the sandbox for the rest of this session.
    state.registry.set_orbs_dir(new_orbs_dir);
    state.library_access.lock().await.replace(new_guard);
    state.library_bookmark_stale.store(false, Ordering::Relaxed);

    Ok(OrbLibraryPick {
        path: pending.path.to_string_lossy().into_owned(),
        pending: false,
        orb_count,
    })
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn apply_orb_library_change() -> Result<OrbLibraryPick, String> {
    Err("Orb library folder selection is only available on macOS.".to_string())
}

/// Abandons a pending library-folder change; settings stay on the old folder.
#[cfg(target_os = "macos")]
#[tauri::command]
async fn cancel_orb_library_change(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.pending_library_pick.lock().await.take();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn cancel_orb_library_change() -> Result<(), String> {
    Err("Orb library folder selection is only available on macOS.".to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn dismiss_onboarding(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let store = state.settings.lock().await;
    let mut settings = store.load().map_err(to_string)?;
    settings.onboarding_complete = true;
    store.save(&settings).map_err(to_string)
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn dismiss_onboarding() -> Result<(), String> {
    Ok(())
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
    let gw_running = state
        .running_children
        .lock()
        .await
        .contains_key(UNIFIED_GATEWAY_KEY);
    // The unified gateway owns `http_port`; per-orb servers shift up to avoid
    // a bind conflict when it is active.
    let port = settings.http_port + if gw_running { 1 } else { 0 };

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
    // macOS sandbox: security-scoped bookmarks are not inherited by child
    // processes; pass the bookmark explicitly so the runtime can resolve access
    // to the user-picked library folder before reading the Orb ZIP.
    #[cfg(target_os = "macos")]
    if let Some(ref bookmark) = settings.orb_library_bookmark {
        if !bookmark.is_empty() {
            cmd.arg("--library-bookmark").arg(bookmark);
        }
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
        let gw_running = children_lock.contains_key(UNIFIED_GATEWAY_KEY);
        let base_port = settings.http_port + if gw_running { 1 } else { 0 };
        let port = base_port + results.len() as u16;

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
        #[cfg(target_os = "macos")]
        if let Some(ref bookmark) = settings.orb_library_bookmark {
            if !bookmark.is_empty() {
                cmd.arg("--library-bookmark").arg(bookmark);
            }
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

/// Bind address for the unified gateway process: `127.0.0.1` for localhost
/// mode, `0.0.0.0` (all interfaces) for external mode.
fn unified_gateway_bind_addr(binding: NetworkBinding) -> &'static str {
    match binding {
        NetworkBinding::Localhost => "127.0.0.1",
        NetworkBinding::External => "0.0.0.0",
    }
}

/// The address MCP clients should connect to. In external mode the gateway
/// binds `0.0.0.0`, but `0.0.0.0` is not a connectable address, so advertise
/// the machine's LAN IP instead (falling back to localhost if undetectable).
fn unified_gateway_url(port: u16, binding: NetworkBinding) -> String {
    let host = match binding {
        NetworkBinding::Localhost => "127.0.0.1".to_string(),
        NetworkBinding::External => local_lan_ip().unwrap_or_else(|| "127.0.0.1".to_string()),
    };
    format!("http://{host}:{port}/mcp")
}

/// First non-loopback IPv4 address, if any.
fn local_lan_ip() -> Option<String> {
    local_ip_address::local_ip()
        .ok()
        .map(|ip| ip.to_string())
}

fn generate_gateway_token() -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

async fn is_port_listening(port: u16) -> bool {
    tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
}

async fn unified_gateway_status_impl(state: &AppState) -> Result<UnifiedGatewayStatus, String> {
    let settings = state.settings.lock().await.load().map_err(to_string)?;
    let port = settings.http_port;
    let running = state
        .running_children
        .lock()
        .await
        .contains_key(UNIFIED_GATEWAY_KEY)
        && is_port_listening(port).await;
    Ok(UnifiedGatewayStatus {
        running,
        port,
        url: unified_gateway_url(port, settings.network_binding),
        token: settings.gateway_token.clone(),
    })
}

async fn ensure_unified_gateway_impl(state: &AppState) -> Result<UnifiedGatewayStatus, String> {
    if state
        .running_children
        .lock()
        .await
        .contains_key(UNIFIED_GATEWAY_KEY)
    {
        return unified_gateway_status_impl(state).await;
    }

    let gateway_bin = default_gateway_http_binary()
        .ok_or_else(|| "mcporb-gateway-http binary not found next to mcporb-runner".to_string())?;

    let (port, token, bind_addr) = {
        let store = state.settings.lock().await;
        let mut settings = store.load().map_err(to_string)?;
        if settings.gateway_token.as_deref().map_or(true, str::is_empty) {
            settings.gateway_token = Some(generate_gateway_token());
            store.save(&settings).map_err(to_string)?;
        }
        (
            settings.http_port,
            settings.gateway_token.clone().unwrap_or_default(),
            unified_gateway_bind_addr(settings.network_binding),
        )
    };
    let registry_root = state.registry.root_dir().to_path_buf();

    let mut cmd = tokio::process::Command::new(&gateway_bin);
    cmd.arg("--registry-dir")
        .arg(&registry_root)
        .arg("--port")
        .arg(port.to_string())
        .arg("--bind")
        .arg(bind_addr)
        .arg("--token")
        .arg(&token)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn {gateway_bin:?}: {e}"))?;

    if let Some(stderr) = child.stderr.take() {
        let reader = tokio::io::BufReader::new(stderr);
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(target: "unified_gateway", "{line}");
            }
        });
    }

    state
        .running_children
        .lock()
        .await
        .insert(UNIFIED_GATEWAY_KEY.to_string(), child);

    for _ in 0..30 {
        if is_port_listening(port).await {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    if !is_port_listening(port).await {
        if let Some(mut c) = state.running_children.lock().await.remove(UNIFIED_GATEWAY_KEY) {
            let _ = c.kill().await;
        }
        return Err(format!(
            "unified gateway failed to listen on {port} (check for a port conflict)"
        ));
    }

    tracing::info!(%port, registry_root = %registry_root.display(), "unified HTTP gateway started");
    unified_gateway_status_impl(state).await
}

async fn stop_unified_gateway_impl(state: &AppState) -> Result<(), String> {
    if let Some(mut child) = state
        .running_children
        .lock()
        .await
        .remove(UNIFIED_GATEWAY_KEY)
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    Ok(())
}

#[tauri::command]
async fn unified_gateway_status(
    state: tauri::State<'_, AppState>,
) -> Result<UnifiedGatewayStatus, String> {
    unified_gateway_status_impl(&state).await
}

#[tauri::command]
async fn ensure_unified_gateway(
    state: tauri::State<'_, AppState>,
) -> Result<UnifiedGatewayStatus, String> {
    ensure_unified_gateway_impl(&state).await
}

#[tauri::command]
async fn reset_gateway_token(
    state: tauri::State<'_, AppState>,
) -> Result<UnifiedGatewayStatus, String> {
    {
        let store = state.settings.lock().await;
        let mut settings = store.load().map_err(to_string)?;
        settings.gateway_token = Some(generate_gateway_token());
        store.save(&settings).map_err(to_string)?;
    }
    stop_unified_gateway_impl(&state).await?;
    ensure_unified_gateway_impl(&state).await
}

#[tauri::command]
async fn stop_unified_gateway(state: tauri::State<'_, AppState>) -> Result<(), String> {
    stop_unified_gateway_impl(&state).await
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
    let host = match settings.network_binding {
        NetworkBinding::Localhost => "127.0.0.1".to_string(),
        NetworkBinding::External => local_lan_ip().unwrap_or_else(|| "127.0.0.1".to_string()),
    };
    Ok(mcp_config::gateway_http_config_snippets(
        settings.http_port,
        settings.gateway_token.as_deref(),
        &host,
    ))
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
        let entry = gateway_server_entry(&registry_dir, &gateway_bin);
        let existing = platform.current_content.as_deref();
        return match platform_config::build_merged_config_with_entries(existing, std::slice::from_ref(&entry)) {
            Ok(json) => json,
            Err(_) => {
                let mut map = serde_json::Map::new();
                let mut servers = serde_json::Map::new();
                servers.insert(entry.0, entry.1);
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
    _state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let client = StoreClient::new().map_err(to_string)?;

    // Use the OS temp dir ($TMPDIR on macOS) which is always sandbox-accessible,
    // rather than settings.download_dir which may point to ~/Downloads where we
    // lack the files.downloads.read-write entitlement.
    let dest_dir = std::env::temp_dir().join("mcporb-store");
    std::fs::create_dir_all(&dest_dir).map_err(to_string)?;
    let dest_path = dest_dir.join(format!("{artifact_id}.zip"));
    let token = token.unwrap_or_default();
    let _size = client
        .download_orb(&artifact_id, &token, &dest_path)
        .await
        .map_err(to_string)?;

    Ok(dest_path.to_string_lossy().to_string())
}

fn default_registry_root() -> PathBuf {
    RegistryStore::default()
        .map(|store| store.root_dir().to_path_buf())
        .unwrap_or_else(|_| PathBuf::from(".mcporb-runtime"))
}

#[cfg(target_os = "macos")]
fn resolve_macos_registry(
    settings: &RuntimeSettings,
) -> (Option<macos_access::AccessGuard>, RegistryStore, bool) {
    let fallback = || RegistryStore::new(default_registry_root());
    match (&settings.orb_library_dir, &settings.orb_library_bookmark) {
        (Some(_), Some(bookmark)) => match macos_access::resolve_bookmark(bookmark) {
            Ok((path, guard)) => {
                tracing::info!(path = %path.display(), "using user-selected Orb library folder");
                (
                    Some(guard),
                    RegistryStore::with_orbs_dir(default_registry_root(), path.join("Orbs")),
                    false,
                )
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    "failed to resolve Orb library bookmark; falling back to app data folder"
                );
                (None, fallback(), true)
            }
        },
        _ => (None, fallback(), false),
    }
}

fn main() {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    let settings_store = SettingsStore::default().unwrap_or_else(|error| {
        tracing::warn!(%error, "falling back to default settings store");
        SettingsStore::new(PathBuf::from(".mcporb-runtime"))
    });

    // macOS: restore security-scoped access to the user-chosen Orb library
    // folder before anything else — the MCP STDIO proxy child needs it too.
    #[cfg(target_os = "macos")]
    let (library_access, registry, bookmark_stale) = {
        let settings = settings_store.load().unwrap_or_default();
        let (access, reg, is_stale) = resolve_macos_registry(&settings);
        
        // A stale/unusable security-scoped bookmark can only be re-created by
        // the user re-selecting the folder (NSOpenPanel grants a fresh
        // extension), so DO NOT wipe orb_library_dir / orb_library_bookmark
        // here. Clearing them silently would permanently orphan every orb in
        // the registry (their zip_paths still point into the library folder),
        // hide where the library lives, and turn a recoverable access loss
        // into a permanent "Operation not permitted" on every launch. Instead
        // keep the settings intact and let the UI (get_library_health → stale
        // banner) prompt the user to re-select; the runtime child still
        // receives the bookmark and reports the actionable "failed to read Orb
        // ZIP … re-grant folder access" error instead of a bare EPERM.
        if is_stale {
            tracing::warn!(
                "Orb library bookmark is stale/unusable; orb access requires re-selecting the folder in Settings (Choose…)"
            );
        }
        
        (access, reg, is_stale)
    };
    #[cfg(not(target_os = "macos"))]
    let registry = RegistryStore::new(default_registry_root());

    // --- MCP STDIO proxy mode ---
    // Intercept before any Tauri/WebKit init to keep stdio clean and avoid
    // the ~50 MB WebView overhead when launched by an MCP client.
    if std::env::args().any(|a| a == "--mcp-stdio") {
        // Re-read settings here (after potential stale-bookmark clear above) to
        // get the current bookmark. None if stale/missing = no --library-bookmark.
        let mcp_library_bookmark = settings_store
            .load()
            .ok()
            .and_then(|s| s.orb_library_bookmark)
            .filter(|b| !b.is_empty());
        run_mcp_stdio_proxy(registry.root_dir(), mcp_library_bookmark);
    }
    if std::env::args().any(|a| a == "--gateway-stdio") {
        run_gateway_stdio_proxy(registry.root_dir());
    }

    let app_state = AppState {
        registry,
        settings: Arc::new(Mutex::new(settings_store)),
        running_orbs: Arc::new(Mutex::new(Vec::new())),
        running_children: Arc::new(Mutex::new(HashMap::new())),
        pending_library_pick: Arc::new(Mutex::new(None)),
        #[cfg(target_os = "macos")]
        library_access: Arc::new(Mutex::new(library_access)),
        #[cfg(target_os = "macos")]
        library_bookmark_stale: Arc::new(AtomicBool::new(bookmark_stale)),
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
        .setup(|app| {
            let state = app.state::<AppState>().inner().clone();
            let gw_state = state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = ensure_unified_gateway_impl(&gw_state).await {
                    tracing::warn!(error = %e, "auto-start of unified HTTP gateway failed");
                }
            });
            // SIGTERM/SIGINT teardown: `kill_on_drop` on child processes does not
            // fire when the process is killed by a signal, which would orphan the
            // gateway child and leave its port occupied, breaking the next
            // auto-start. Tear children down explicitly before exiting.
            #[cfg(unix)]
            {
                let state = state.clone();
                tauri::async_runtime::spawn(async move {
                    use tokio::signal::unix::{signal, SignalKind};
                    let mut term = signal(SignalKind::terminate())
                        .expect("register SIGTERM handler");
                    let mut int = signal(SignalKind::interrupt())
                        .expect("register SIGINT handler");
                    tokio::select! {
                        _ = term.recv() => {}
                        _ = int.recv() => {}
                    }
                    tracing::info!("received termination signal; shutting down children");
                    shutdown_running_orbs(&state).await;
                    std::process::exit(0);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runtime_status,
            get_platform,
            get_library_health,
            choose_orb_library_dir,
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
            unified_gateway_status,
            ensure_unified_gateway,
            stop_unified_gateway,
            reset_gateway_token,
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
            choose_orb_library_dir_suggested,
            dismiss_onboarding,
            apply_orb_library_change,
            cancel_orb_library_change,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCPOrb Runner");
}

/// Exit without running atexit() handlers.
///
/// On macOS, mcporb-runner links against AppKit/WebKit (via Tauri). AppKit
/// registers atexit() callbacks that crash when NSApp was never initialized —
/// which is the case in --mcp-stdio and --gateway-stdio proxy modes where we
/// intentionally bypass Tauri/AppKit. std::process::exit() calls libc exit()
/// which runs those handlers; _exit() does not.
#[cfg(unix)]
fn proxy_exit(code: i32) -> ! {
    extern "C" {
        fn _exit(status: i32) -> !;
    }
    unsafe { _exit(code) }
}

/// Run as an MCP STDIO proxy.
///
/// On Unix: exec() replaces the current process image with mcporb-runtime,
/// inheriting all file descriptors. The MCP client communicates directly with
/// the runtime; mcporb-runner's AppKit atexit handlers never run.
///
/// On Windows: spawn + wait (no AppKit concern on Windows).
fn run_mcp_stdio_proxy(registry_root: &Path, library_bookmark: Option<String>) -> ! {
    let runtime_path = default_runtime_binary().unwrap_or_else(|| {
        eprintln!("mcporb-runner: mcporb-runtime binary not found");
        #[cfg(unix)]
        proxy_exit(1);
        #[cfg(not(unix))]
        std::process::exit(1);
    });

    let metrics_dir = registry_root.join("metrics");

    let mut runtime_args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--mcp-stdio")
        .collect();

    if !runtime_args.iter().any(|a| a == "--stdio-only") {
        runtime_args.push("--stdio-only".to_string());
    }
    if !runtime_args.iter().any(|a| a == "--metrics-dir") {
        runtime_args.push("--metrics-dir".to_string());
        runtime_args.push(metrics_dir.display().to_string());
    }
    // macOS sandbox: security-scoped bookmarks are not inherited across exec();
    // inject the bookmark so the runtime can open the user-picked library folder.
    if let Some(ref bookmark) = library_bookmark {
        if !runtime_args.iter().any(|a| a == "--library-bookmark") {
            runtime_args.push("--library-bookmark".to_string());
            runtime_args.push(bookmark.clone());
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&runtime_path)
            .args(&runtime_args)
            .exec();
        eprintln!("mcporb-runner: exec mcporb-runtime failed: {err}");
        proxy_exit(1);
    }

    #[cfg(not(unix))]
    {
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
}

/// Run as an MCP gateway STDIO proxy.
///
/// On Unix: exec() replaces the current process image with mcporb-gateway-stdio.
/// On Windows: spawn + wait.
fn run_gateway_stdio_proxy(registry_root: &Path) -> ! {
    let gateway_path = default_gateway_binary().unwrap_or_else(|| {
        eprintln!("mcporb-runner: mcporb-gateway-stdio binary not found");
        #[cfg(unix)]
        proxy_exit(1);
        #[cfg(not(unix))]
        std::process::exit(1);
    });

    let mut gateway_args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--gateway-stdio")
        .collect();

    if !gateway_args.iter().any(|a| a == "--registry-dir") {
        gateway_args.push("--registry-dir".to_string());
        gateway_args.push(registry_root.display().to_string());
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&gateway_path)
            .args(&gateway_args)
            .exec();
        eprintln!("mcporb-runner: exec mcporb-gateway-stdio failed: {err}");
        proxy_exit(1);
    }

    #[cfg(not(unix))]
    {
        let mut child = match std::process::Command::new(&gateway_path)
            .args(&gateway_args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("mcporb-runner: failed to spawn mcporb-gateway-stdio: {e}");
                std::process::exit(1);
            }
        };
        let status = child.wait().unwrap_or_else(|e| {
            eprintln!("mcporb-runner: failed to wait for mcporb-gateway-stdio: {e}");
            std::process::exit(1);
        });
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(target_os = "macos")]
fn gateway_server_entry(registry_dir: &Path, gateway_bin: &Path) -> (String, serde_json::Value) {
    let parent = gateway_bin.parent().unwrap_or_else(|| Path::new("."));
    let runner = resolve_runner_binary_in(parent).unwrap_or_else(|| {
        parent.join(format!("mcporb-runner{}", std::env::consts::EXE_SUFFIX))
    });
    (
        "mcporb-gateway".to_string(),
        serde_json::json!({
            "command": runner.display().to_string(),
            "args": [
                "--gateway-stdio",
                "--registry-dir",
                registry_dir.display().to_string()
            ]
        }),
    )
}

#[cfg(not(target_os = "macos"))]
fn gateway_server_entry(registry_dir: &Path, gateway_bin: &Path) -> (String, serde_json::Value) {
    (
        "mcporb-gateway".to_string(),
        serde_json::json!({
            "command": gateway_bin.display().to_string(),
            "args": [
                "--registry-dir",
                registry_dir.display().to_string()
            ]
        }),
    )
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

fn resolve_runner_binary_in(dir: &std::path::Path) -> Option<PathBuf> {
    let suffix = std::env::consts::EXE_SUFFIX;
    let runner = dir.join(format!("mcporb-runner{suffix}"));
    if runner.is_file() {
        Some(runner)
    } else {
        None
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

/// Locate the unified HTTP gateway binary (`mcporb-gateway-http`) next to the
/// current executable. Bundled into the .app by `build.rs` `sync_external_binaries`.
fn resolve_gateway_http_binary_in(dir: &std::path::Path) -> Option<PathBuf> {
    let suffix = std::env::consts::EXE_SUFFIX;
    let path = dir.join(format!("mcporb-gateway-http{suffix}"));
    if path.is_file() { Some(path) } else { None }
}

/// The path to the `mcporb-gateway-http` binary, resolved relative to the
/// current executable.
fn default_gateway_http_binary() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    resolve_gateway_http_binary_in(&dir)
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

    // ── settings merge tests ────────────────────────────────────────────────

    #[test]
    fn merge_settings_preserves_gateway_token_when_incoming_is_empty() {
        let mut current = RuntimeSettings::default();
        current.gateway_token = Some("persistent-token".to_string());
        // Incoming settings from the frontend form carry no token (None).
        let merged = merge_settings(current, RuntimeSettings::default());
        assert_eq!(merged.gateway_token.as_deref(), Some("persistent-token"));
    }

    #[test]
    fn merge_settings_honors_an_explicit_new_token() {
        let mut current = RuntimeSettings::default();
        current.gateway_token = Some("old".to_string());
        let mut incoming = RuntimeSettings::default();
        incoming.gateway_token = Some("new".to_string());
        let merged = merge_settings(current, incoming);
        assert_eq!(merged.gateway_token.as_deref(), Some("new"));
    }

    #[test]
    fn merge_settings_preserves_orb_library_bookmark() {
        let mut current = RuntimeSettings::default();
        current.orb_library_dir = Some(PathBuf::from("/Users/t/Documents/MCPOrb"));
        current.orb_library_bookmark = Some("bm".to_string());
        let merged = merge_settings(current, RuntimeSettings::default());
        assert_eq!(merged.orb_library_dir.as_deref(), Some(std::path::Path::new("/Users/t/Documents/MCPOrb")));
        assert_eq!(merged.orb_library_bookmark.as_deref(), Some("bm"));
    }

    #[test]
    fn merge_settings_preserves_bookmark_when_dir_is_unchanged() {
        // The frontend form always submits the (read-only) library dir field,
        // so the incoming orb_library_dir is Some and equal to the current
        // one. The bookmark itself must still survive an unrelated save.
        let mut current = RuntimeSettings::default();
        current.orb_library_dir = Some(PathBuf::from("/Users/t/Documents/MCPOrb"));
        current.orb_library_bookmark = Some("bm".to_string());
        let mut incoming = RuntimeSettings::default();
        incoming.orb_library_dir = Some(PathBuf::from("/Users/t/Documents/MCPOrb"));
        let merged = merge_settings(current, incoming);
        assert_eq!(merged.orb_library_bookmark.as_deref(), Some("bm"));
    }

    #[test]
    fn merge_settings_drops_bookmark_when_dir_changes() {
        // A genuinely different library dir without a bookmark must not keep
        // the stale bookmark of the previous folder.
        let mut current = RuntimeSettings::default();
        current.orb_library_dir = Some(PathBuf::from("/Users/t/Documents/MCPOrb"));
        current.orb_library_bookmark = Some("bm".to_string());
        let mut incoming = RuntimeSettings::default();
        incoming.orb_library_dir = Some(PathBuf::from("/Volumes/SSD/OtherLib"));
        let merged = merge_settings(current, incoming);
        assert_eq!(merged.orb_library_dir.as_deref(), Some(std::path::Path::new("/Volumes/SSD/OtherLib")));
        assert_eq!(merged.orb_library_bookmark, None);
    }

    #[test]
    fn generate_gateway_token_is_url_safe_and_random() {
        let a = generate_gateway_token();
        let b = generate_gateway_token();
        // 32 random bytes, base64url no padding → 43 chars, chars ∈ [A-Za-z0-9-_].
        assert_eq!(a.len(), 43);
        assert!(a.bytes().all(|c| matches!(c, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_')));
        assert_ne!(a, b, "two independent draws must not collide");
    }

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
    fn resolve_gateway_http_binary_finds_http_not_stdio() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let bin = dir.path().join(format!("mcporb-gateway-http{suffix}"));
        fs::write(&bin, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolve_gateway_http_binary_in(dir.path()), Some(bin));
        assert_eq!(resolve_gateway_binary_in(dir.path()), None);
    }

    #[test]
    fn resolve_gateway_http_binary_absent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let bin = dir.path().join(format!("mcporb-gateway-stdio{suffix}"));
        fs::write(&bin, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolve_gateway_http_binary_in(dir.path()), None);
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

    #[test]
    fn resolve_runner_binary_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve_runner_binary_in(dir.path()), None);
    }

    #[test]
    fn resolve_runner_binary_finds_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let runner = dir.path().join(format!("mcporb-runner{suffix}"));
        fs::write(&runner, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolve_runner_binary_in(dir.path()), Some(runner));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn gateway_server_entry_uses_runner_wrapper_on_macos() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let gateway = dir.path().join(format!("mcporb-gateway-stdio{suffix}"));
        let runner = dir.path().join(format!("mcporb-runner{suffix}"));
        fs::write(&gateway, "").unwrap();
        fs::write(&runner, "").unwrap();
        #[cfg(unix)]
        {
            fs::set_permissions(&gateway, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let entry = gateway_server_entry(Path::new("/tmp/mcporb-runtime"), &gateway);
        assert_eq!(entry.0, "mcporb-gateway");
        assert_eq!(entry.1["command"], runner.display().to_string());
        assert_eq!(entry.1["args"][0], "--gateway-stdio");
        assert_eq!(entry.1["args"][1], "--registry-dir");
        assert_eq!(entry.1["args"][2], "/tmp/mcporb-runtime");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn gateway_server_entry_keeps_gateway_direct_on_non_macos() {
        let dir = tempfile::tempdir().unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let gateway = dir.path().join(format!("mcporb-gateway-stdio{suffix}"));
        fs::write(&gateway, "").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&gateway, fs::Permissions::from_mode(0o755)).unwrap();

        let entry = gateway_server_entry(Path::new("/tmp/mcporb-runtime"), &gateway);
        assert_eq!(entry.0, "mcporb-gateway");
        assert_eq!(entry.1["command"], gateway.display().to_string());
        assert_eq!(entry.1["args"][0], "--registry-dir");
        assert_eq!(entry.1["args"][1], "/tmp/mcporb-runtime");
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
                pending_library_pick: Arc::new(Mutex::new(None)),
                #[cfg(target_os = "macos")]
                library_access: Arc::new(Mutex::new(None)),
                #[cfg(target_os = "macos")]
                library_bookmark_stale: Arc::new(AtomicBool::new(false)),
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

    // ── reset-gateway-token dialog regression tests ─────────────────────────
    //
    // The Runner frontend is plain JS (no bundler), so the dialog plugin's
    // guest-JS global `window.__TAURI__.dialog` is never exposed, and the
    // plugin's init script overrides `window.confirm` to call a nonexistent
    // `plugin:dialog|confirm` command. The reset-token button must call the
    // real `plugin:dialog|message` command over IPC instead; these tests pin
    // that contract so the button can never silently die again.

    #[test]
    fn reset_token_button_uses_direct_message_ipc_in_frontend() {
        let app_js = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend/app.js"),
        )
        .expect("frontend/app.js must exist next to the crate");
        assert!(
            app_js.contains("invoke('plugin:dialog|message'"),
            "reset-token handler must call plugin:dialog|message directly"
        );
        assert!(
            app_js.contains("buttons: 'OkCancel'"),
            "reset-token confirm must use OkCancel buttons"
        );
        assert!(
            !app_js.contains("window.__TAURI__?.dialog"),
            "frontend must not depend on the unexposed window.__TAURI__.dialog global"
        );
    }

    #[test]
    fn capabilities_grant_dialog_message_permission() {
        let caps_json = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json"),
        )
        .expect("capabilities/default.json must exist next to the crate");
        let caps: serde_json::Value =
            serde_json::from_str(&caps_json).expect("capabilities/default.json must be valid JSON");
        let granted: Vec<&str> = caps["permissions"]
            .as_array()
            .expect("permissions must be an array")
            .iter()
            .filter_map(|p| p.as_str())
            .collect();
        assert!(
            granted.contains(&"dialog:allow-message"),
            "dialog:allow-message must be granted for the confirm dialog; got {granted:?}"
        );
        assert!(
            granted.contains(&"dialog:allow-open"),
            "dialog:allow-open must be granted for the file picker; got {granted:?}"
        );
    }

    #[test]
    fn frontend_library_change_modal_wired_in_html_and_js() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let app_js = fs::read_to_string(manifest_dir.join("frontend/app.js"))
            .expect("frontend/app.js must exist next to the crate");
        let index_html = fs::read_to_string(manifest_dir.join("frontend/index.html"))
            .expect("frontend/index.html must exist next to the crate");

        assert!(
            index_html.contains("id=\"library-change-modal\""),
            "index.html must contain the library-change modal"
        );
        assert!(
            index_html.contains("btn-library-change-migrate"),
            "index.html must contain the migrate button"
        );
        assert!(
            index_html.contains("btn-library-change-delete"),
            "index.html must contain the delete button"
        );
        assert!(
            app_js.contains("invoke('choose_orb_library_dir')"),
            "frontend must open the folder picker via choose_orb_library_dir"
        );
        assert!(
            app_js.contains("invoke('apply_orb_library_change', { action: 'migrate' })"),
            "migrate action must call apply_orb_library_change"
        );
        assert!(
            app_js.contains("invoke('apply_orb_library_change', { action: 'delete' })"),
            "delete action must call apply_orb_library_change"
        );
    }

    // ── gateway network-binding tests ───────────────────────────────────────

    #[test]
    fn gateway_bind_addr_follows_network_binding() {
        assert_eq!(unified_gateway_bind_addr(NetworkBinding::Localhost), "127.0.0.1");
        assert_eq!(unified_gateway_bind_addr(NetworkBinding::External), "0.0.0.0");
    }

    #[test]
    fn gateway_url_localhost_mode_uses_loopback() {
        assert_eq!(
            unified_gateway_url(5599, NetworkBinding::Localhost),
            "http://127.0.0.1:5599/mcp"
        );
    }

    #[test]
    fn gateway_url_external_mode_advertises_connectable_host() {
        // 0.0.0.0 is not connectable, so external mode must advertise a real
        // host — the LAN IP when available, loopback as a last resort.
        let url = unified_gateway_url(5599, NetworkBinding::External);
        assert!(url.starts_with("http://"), "unexpected url: {url}");
        assert!(url.ends_with(":5599/mcp"), "unexpected url: {url}");
        assert!(!url.contains("0.0.0.0"), "external url must not be 0.0.0.0: {url}");
    }
}
