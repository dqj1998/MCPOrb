use crate::{api, assets::WebAssets, mcp_handler, state::SharedState};
use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64_url_encode(&bytes)
}

fn base64_url_encode(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as usize;
        let b1 = if i + 1 < bytes.len() {
            bytes[i + 1] as usize
        } else {
            0
        };
        let b2 = if i + 2 < bytes.len() {
            bytes[i + 2] as usize
        } else {
            0
        };
        result.push(CHARS[b0 >> 2] as char);
        result.push(CHARS[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if i + 1 < bytes.len() {
            result.push(CHARS[((b1 & 15) << 2) | (b2 >> 6)] as char);
        }
        if i + 2 < bytes.len() {
            result.push(CHARS[b2 & 63] as char);
        }
        i += 3;
    }
    result
}

async fn validate_host(headers: HeaderMap, request: Request, next: Next) -> Response {
    if let Some(host) = headers.get("host").and_then(|h| h.to_str().ok()) {
        let h = host.to_lowercase();
        if h.starts_with("127.0.0.1:") || h.starts_with("localhost:") {
            return next.run(request).await;
        }
    }
    StatusCode::FORBIDDEN.into_response()
}

/// Gate for protected API routes: 401 unless the process is unlocked (plan §4.3).
/// A single unlock (here or via the stdio MCP `unlock_orb`) opens all surfaces.
async fn require_unlocked(
    axum::extract::State(state): axum::extract::State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    if state.security.is_unlocked() {
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

pub async fn serve(
    state: SharedState,
    port: Option<u16>,
    token: &str,
    bind_external: bool,
) -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let token = token.to_string();

    let index_html = WebAssets::get("index.html")
        .map(|f| String::from_utf8_lossy(f.data.as_ref()).to_string())
        .unwrap_or_else(|| "<html><body><h1>MCPOrb</h1></body></html>".to_string());

    // Protected API: everything that exposes knowledge or config actions. Gated
    // by the unlock middleware via `route_layer`, so the layer applies only to
    // these routes — never the auth routes below (plan §4.3).
    let protected = Router::new()
        .route("/manifest", get(api::get_manifest))
        .route("/documents", get(api::get_documents))
        .route("/metrics", get(api::get_metrics))
        .route("/metrics/qa", post(api::get_qa_history))
        .route("/mcp-config-locations", get(api::get_mcp_config_locations))
        .route(
            "/open-mcp-config-location",
            post(api::post_open_mcp_config_location),
        )
        .route("/search", post(api::post_search))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_unlocked,
        ))
        .with_state(state.clone());

    // Auth API: unauthenticated so the gate can be queried and lifted.
    let auth_router = Router::new()
        .route("/auth/status", get(api::get_auth_status))
        .route("/auth/unlock", post(api::post_auth_unlock))
        .with_state(state.clone());

    let api_router = protected.merge(auth_router);

    let html_clone = index_html.clone();
    let token_for_redirect = token.clone();

    let mut app = Router::new()
        .nest(&format!("/{token}/api"), api_router)
        .route(
            &format!("/{token}/"),
            get(move || {
                let h = html_clone.clone();
                async move { Html(h) }
            }),
        )
        .route(
            &format!("/{token}"),
            get(move || {
                let url = format!("/{token_for_redirect}/");
                async move { axum::response::Redirect::permanent(&url) }
            }),
        )
        .route(
            &format!("/{token}/mcp"),
            post(mcp_handler::post_streamable_http_mcp),
        )
        .fallback(|| async { StatusCode::NOT_FOUND })
        .with_state(state);

    // Restrict to loopback when not in external binding mode.
    if !bind_external {
        app = app.layer(middleware::from_fn(validate_host));
    }

    let bind_ip = if bind_external { "0.0.0.0" } else { "127.0.0.1" };
    let bind_addr = format!("{bind_ip}:{}", port.unwrap_or(0));
    let listener = TcpListener::bind(&bind_addr).await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok((addr, handle))
}
