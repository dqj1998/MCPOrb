use crate::mcp_handler::auto_fill_query_vector;
use crate::state::SharedState;
use axum::{extract::State, http::StatusCode, response::Json};
use mcporb_runtime_core::{SearchMethodRequest, SearchRequest as RuntimeSearchRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

/// `GET /api/auth/status` — unauthenticated. Lets the Web UI decide whether to
/// show the login gate before any protected call (plan §3.1, §4.3).
pub async fn get_auth_status(State(state): State<SharedState>) -> Json<Value> {
    Json(json!({
        "password_required": state.security.password_required(),
        "unlocked": state.security.is_unlocked(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct UnlockRequest {
    pub password: String,
}

/// `POST /api/auth/unlock` — unauthenticated. Verifies the password and, in
/// password-only mode, unlocks the whole process. On failure applies the
/// throttle delay (plan §2.6) and returns a generic 401.
pub async fn post_auth_unlock(
    State(state): State<SharedState>,
    Json(req): Json<UnlockRequest>,
) -> (StatusCode, Json<Value>) {
    if state.security.is_unlocked() {
        return (StatusCode::OK, Json(json!({ "unlocked": true })));
    }
    match crate::perform_unlock(&state, &req.password) {
        // perform_unlock verifies the password and, for encrypted Orbs, decrypts
        // and publishes the knowledge before reporting unlocked.
        Ok(()) => (StatusCode::OK, Json(json!({ "unlocked": true }))),
        Err(_) => {
            let delay = state.security.backoff_delay();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "unlocked": false, "error": "Invalid password" })),
            )
        }
    }
}

pub async fn get_manifest(State(state): State<SharedState>) -> Json<Value> {
    let k = state.knowledge();
    Json(json!({
        "name": k.manifest.name,
        "version": k.manifest.version,
        "description": k.manifest.description,
        "chunk_count": k.manifest.chunk_count,
        "startup_mode": state.startup_mode,
        "enabled_capabilities": k.search.capabilities().iter().map(|c| format!("{c:?}").to_lowercase()).collect::<Vec<_>>(),
        "available_methods": k.search.available_method_names(),
        "orb_binary_path": state.orb_binary_path,
    }))
}

pub async fn get_documents(State(state): State<SharedState>) -> Json<Value> {
    let docs: Vec<Value> = state
        .knowledge()
        .documents
        .iter()
        .map(|d| {
            json!({
                "id": d.id,
                "format": document_format(&d.source_path),
                "page_count": d.page_count,
                "section_count": d.sections.len(),
            })
        })
        .collect();
    Json(json!({ "documents": docs }))
}

fn document_format(source_path: &str) -> String {
    std::path::Path::new(source_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_uppercase())
        .filter(|ext| !ext.is_empty())
        .unwrap_or_else(|| "Document".to_string())
}

pub async fn get_metrics(State(state): State<SharedState>) -> Json<Value> {
    let metrics = state.metrics.read().await;
    Json(json!({
        "mcp_request_count": metrics.mcp_request_count,
        "search_count": metrics.search_count,
        "startup_mode": state.startup_mode,
    }))
}

#[derive(Debug, Clone, Serialize)]
pub struct McpConfigLocation {
    pub client: &'static str,
    pub label: &'static str,
    pub file_name: String,
    pub display_path: String,
    pub exists: bool,
}

#[derive(Debug, Deserialize)]
pub struct OpenMcpConfigRequest {
    pub client: String,
}

pub async fn get_mcp_config_locations() -> Json<Value> {
    Json(json!({
        "os": std::env::consts::OS,
        "locations": mcp_config_locations(),
    }))
}

pub async fn post_open_mcp_config_location(
    Json(req): Json<OpenMcpConfigRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let Some((_, path)) = mcp_config_path(&req.client) else {
        return Err((StatusCode::BAD_REQUEST, "unknown MCP client".to_string()));
    };

    open_config_location(path).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to open config location: {error}"),
        )
    })?;
    Ok(StatusCode::NO_CONTENT)
}

fn mcp_config_locations() -> Vec<McpConfigLocation> {
    ["claude_desktop", "cursor", "vscode", "windsurf"]
        .into_iter()
        .filter_map(|client| {
            let (label, path) = mcp_config_path(client)?;
            Some(McpConfigLocation {
                client,
                label,
                file_name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("settings.json")
                    .to_string(),
                display_path: display_home_path(&path),
                exists: path.is_file(),
            })
        })
        .collect()
}

fn mcp_config_path(client: &str) -> Option<(&'static str, PathBuf)> {
    let home = dirs::home_dir()?;

    #[cfg(target_os = "macos")]
    let path = match client {
        "claude_desktop" => {
            home.join("Library/Application Support/Claude/claude_desktop_config.json")
        }
        "cursor" => home.join("Library/Application Support/Cursor/User/settings.json"),
        "vscode" => home.join("Library/Application Support/Code/User/settings.json"),
        "windsurf" => home.join(".codeium/windsurf/mcp_config.json"),
        _ => return None,
    };

    #[cfg(target_os = "windows")]
    let path = {
        let app_data = dirs::config_dir().unwrap_or_else(|| home.join("AppData/Roaming"));
        match client {
            "claude_desktop" => app_data.join("Claude/claude_desktop_config.json"),
            "cursor" => app_data.join("Cursor/User/settings.json"),
            "vscode" => app_data.join("Code/User/settings.json"),
            "windsurf" => home.join(".codeium/windsurf/mcp_config.json"),
            _ => return None,
        }
    };

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let path = {
        let config_dir = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
        match client {
            "claude_desktop" => config_dir.join("Claude/claude_desktop_config.json"),
            "cursor" => config_dir.join("Cursor/User/settings.json"),
            "vscode" => config_dir.join("Code/User/settings.json"),
            "windsurf" => home.join(".codeium/windsurf/mcp_config.json"),
            _ => return None,
        }
    };

    let label = match client {
        "claude_desktop" => "Claude Desktop",
        "cursor" => "Cursor",
        "vscode" => "VS Code",
        "windsurf" => "Windsurf",
        _ => return None,
    };
    Some((label, path))
}

fn display_home_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

fn open_config_location(path: PathBuf) -> std::io::Result<()> {
    let target = if path.exists() {
        path
    } else {
        nearest_existing_parent(&path).unwrap_or(path)
    };

    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        if target.is_file() {
            command.arg("-R").arg(target);
        } else {
            command.arg(target);
        }
        command.spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("explorer");
        if target.is_file() {
            command.arg(format!("/select,{}", target.display()));
        } else {
            command.arg(target);
        }
        command.spawn()?;
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        std::process::Command::new("xdg-open").arg(target).spawn()?;
    }

    Ok(())
}

fn nearest_existing_parent(path: &std::path::Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|candidate| candidate.exists())
        .map(PathBuf::from)
}

#[derive(serde::Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub top_k: Option<usize>,
    pub method: Option<String>,
    pub query_vector: Option<Vec<f32>>,
}

pub async fn post_search(
    State(state): State<SharedState>,
    Json(req): Json<SearchRequest>,
) -> Json<Value> {
    let top_k = req.top_k.unwrap_or(5);
    let method_name = req.method.as_deref().unwrap_or("auto");
    {
        let mut metrics = state.metrics.write().await;
        metrics.search_count += 1;
    }

    let k = state.knowledge();
    let available_methods = k.search.available_method_names();
    if !available_methods.iter().any(|value| *value == method_name) {
        return Json(json!({
            "query": req.query,
            "error": format!("Unsupported method: {method_name}"),
            "available_methods": available_methods,
        }));
    }

    let requested_method = SearchMethodRequest::from_str(method_name);
    let prepared = match auto_fill_query_vector(
        &state,
        requested_method,
        &req.query,
        req.query_vector.clone(),
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(msg) => {
            return Json(json!({
                "query": req.query,
                "method": method_name,
                "error": msg,
            }))
        }
    };

    match k.search.search(&RuntimeSearchRequest {
        query: req.query.clone(),
        top_k,
        method: prepared.method.clone(),
        query_vector: prepared.query_vector,
        explain: false,
    }) {
        Ok(response) => {
            let hits: Vec<Value> = response
                .hits
                .iter()
                .filter_map(|result| {
                    k.chunks.get(result.chunk_id as usize).map(|chunk| {
                        json!({
                            "chunk_id": chunk.id,
                            "score": result.score,
                            "method": result.method.to_string(),
                            "page": chunk.page,
                            "section_id": chunk.section_id,
                            "text": chunk.text,
                            "token_count": chunk.token_count,
                        })
                    })
                })
                .collect();

            Json(json!({
                "query": req.query,
                "method": prepared.method.as_str(),
                "active_plan": response.active_plan.to_string(),
                "metadata": prepared.metadata,
                "hits": hits,
                "total": hits.len()
            }))
        }
        Err(error) => Json(json!({
            "query": req.query,
            "method": method_name,
            "error": error.to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::state::OrbState;
    use axum::extract::State;
    use mcporb_runtime_core::format::{
        Capability, Chunk, Document, OrbManifest, RetrievalPlanKind,
    };
    use mcporb_runtime_core::{build_bm25_index, DenseRuntime, SearchRuntime};

    fn test_state() -> SharedState {
        state_with_security(crate::security::SecurityConfig::disabled())
    }

    fn state_with_security(security: crate::security::SecurityConfig) -> SharedState {
        let chunks = vec![Chunk {
            id: 0,
            document_id: 0,
            section_id: None,
            page: Some(1),
            text: "model driven architecture guide and platform independent model".to_string(),
            token_count: 8,
        }];
        let documents = vec![Document {
            id: 0,
            title: "Acme Secret Source Filename".to_string(),
            source_path: "/private/customer/Acme Secret Source.pdf".to_string(),
            page_count: Some(12),
            sections: vec![],
        }];
        let manifest = OrbManifest {
            name: "test-orb".to_string(),
            version: "0.1.0".to_string(),
            description: String::new(),
            orb_format_version: "0.2".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            build_time: "2026-06-02T00:00:00Z".to_string(),
            source_documents: vec!["/private/customer/Acme Secret Source.pdf".to_string()],
            chunk_count: chunks.len(),
            index_format_version: "0.2".to_string(),
            binary_size_target_mb: 20,
            selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
            enabled_capabilities: vec![Capability::Bm25],
            embedding_dim: None,
            embedding_model: None,
            embedding_model_tar_sha256: None,
            trigram_min_df: None,
            planning_rationale: vec![json!("internal planning detail")],
        };
        let search = SearchRuntime {
            bm25: build_bm25_index(&chunks),
            tfidf: None,
            trigram: None,
            dense: DenseRuntime::None,
            dense_tier: RetrievalPlanKind::Bm25Only,
        };

        OrbState::new(
            security,
            crate::state::LoadedAssets::Plain(crate::state::LoadedKnowledge {
                manifest,
                documents,
                chunks,
                search,
            }),
            #[cfg(feature = "vector-embedder")]
            std::sync::Arc::new(mcporb_embed::ModelManager::with_cache_dir(
                tempfile::tempdir().unwrap().path().to_path_buf(),
            )),
            #[cfg(feature = "vector-embedder")]
            std::sync::Arc::new(mcporb_embed::empty_slot()),
            "GuiOnly".to_string(),
            Some("/tmp/test.orb".to_string()),
            None,
        )
    }

    #[tokio::test]
    async fn web_search_auto_returns_hits() {
        let state = test_state();
        let Json(response) = post_search(
            State(state),
            Json(SearchRequest {
                query: "model architecture".to_string(),
                top_k: Some(3),
                method: None,
                query_vector: None,
            }),
        )
        .await;

        assert_eq!(response["error"], Value::Null);
        assert_eq!(response["method"], "auto");
        assert_eq!(response["total"], 1);
        assert_eq!(
            response["hits"][0]["text"],
            "model driven architecture guide and platform independent model"
        );
    }

    #[tokio::test]
    async fn documents_api_exposes_format_not_source_names() {
        let state = test_state();
        let Json(response) = get_documents(State(state)).await;
        let doc = &response["documents"][0];

        assert_eq!(doc["format"], "PDF");
        assert_eq!(doc["page_count"], 12);
        assert!(doc.get("title").is_none());
        assert!(doc.get("source_path").is_none());
    }

    #[test]
    fn mcp_config_locations_include_known_clients() {
        let clients: Vec<_> = mcp_config_locations()
            .into_iter()
            .map(|location| location.client)
            .collect();

        assert!(clients.contains(&"claude_desktop"));
        assert!(clients.contains(&"cursor"));
        assert!(clients.contains(&"vscode"));
        assert!(clients.contains(&"windsurf"));
    }

    #[tokio::test]
    async fn auth_status_reports_locked_then_unlocked() {
        // Contract (§4.3): status drives the Web UI gate; flips after unlock.
        let state = state_with_security(crate::security::test_password_config("pw-strong-aaaa"));
        let Json(before) = get_auth_status(State(state.clone())).await;
        assert_eq!(before["password_required"], true);
        assert_eq!(before["unlocked"], false);

        let (code, _) = post_auth_unlock(
            State(state.clone()),
            Json(UnlockRequest {
                password: "pw-strong-aaaa".to_string(),
            }),
        )
        .await;
        assert_eq!(code, StatusCode::OK);

        let Json(after) = get_auth_status(State(state)).await;
        assert_eq!(after["unlocked"], true);
    }

    #[tokio::test]
    async fn wrong_password_unlock_is_unauthorized_and_stays_locked() {
        // Contract: a bad password returns 401 and does not unlock.
        let state = state_with_security(crate::security::test_password_config("pw-strong-aaaa"));
        let (code, Json(body)) = post_auth_unlock(
            State(state.clone()),
            Json(UnlockRequest {
                password: "wrong".to_string(),
            }),
        )
        .await;
        assert_eq!(code, StatusCode::UNAUTHORIZED);
        assert_eq!(body["unlocked"], false);
        assert!(!state.security.is_unlocked());
    }

    #[tokio::test]
    async fn disabled_security_reports_unlocked() {
        // Contract: an Orb with no password needs no unlock.
        let Json(status) = get_auth_status(State(test_state())).await;
        assert_eq!(status["password_required"], false);
        assert_eq!(status["unlocked"], true);
    }
}
