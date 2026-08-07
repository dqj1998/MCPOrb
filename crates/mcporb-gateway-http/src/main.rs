//! MCPOrb Gateway — HTTP transport.
//!
//! Serves a single HTTP MCP endpoint at `POST /mcp` that routes JSON-RPC
//! requests to the appropriate Orb child process.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use clap::Parser;
use mcporb_gateway_core::handler::handle_request;
use mcporb_gateway_core::registry_reader::{discover_orbs, GatewayConfig};
use mcporb_gateway_core::runtime_manager::RuntimeManager;
use tokio::net::TcpListener;
use tracing;

#[derive(Parser, Debug)]
#[command(
    name = "mcporb-gateway-http",
    about = "MCPOrb Gateway — single HTTP MCP endpoint routing to multiple Orbs"
)]
struct Args {
    #[arg(long)]
    registry_dir: Option<PathBuf>,

    #[arg(long)]
    runtime_binary: Option<PathBuf>,

    #[arg(long, default_value = "5600")]
    port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    bind: String,

    #[arg(long, default_value = "300")]
    idle_timeout: u64,

    #[arg(long, default_value = "30")]
    check_interval: u64,

    /// Optional bearer token. When set, every request must present it via
    /// `Authorization: Bearer <token>` or `?token=<token>`.
    #[arg(long)]
    token: Option<String>,
}

fn resolve_config(args: &Args) -> GatewayConfig {
    let mut config = GatewayConfig::default();
    if let Some(dir) = &args.registry_dir {
        config.registry_dir = dir.clone();
    }
    if let Some(bin) = &args.runtime_binary {
        config.runtime_binary = bin.clone();
    }
    config.idle_timeout_secs = args.idle_timeout;
    config.check_interval_secs = args.check_interval;
    config.http_port = args.port;
    config.mcp_transport = "http".to_string();
    config
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let config = resolve_config(&args);

    tracing::info!(
        registry_dir = %config.registry_dir.display(),
        runtime_binary = %config.runtime_binary.display(),
        port = args.port,
        bind = %args.bind,
        "MCPOrb Gateway (HTTP) starting"
    );

    let orbs = match discover_orbs(&config) {
        Ok(orbs) => {
            tracing::info!(count = orbs.len(), "Orbs discovered");
            orbs
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to read registry, starting empty");
            vec![]
        }
    };

    let manager = Arc::new(RuntimeManager::new(config, orbs));

    let tool_count: usize = manager.list_orbs().iter().map(|o| o.tools.len()).sum();
    tracing::info!(
        orb_count = manager.list_orbs().len(),
        tool_count = tool_count,
        "Gateway ready"
    );

    let reaper = manager.clone();
    tokio::spawn(async move {
        reaper.start_reaper().await;
    });

    let state = Arc::new(GatewayState {
        manager,
        token: args.token.map(Arc::from),
    });

    let app = build_app(state);

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind address: {e}"))?;

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(addr = %addr, "HTTP server listening");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Shared gateway state: the routing core plus an optional auth token.
struct GatewayState {
    manager: Arc<RuntimeManager>,
    token: Option<Arc<str>>,
}

fn build_app(state: Arc<GatewayState>) -> Router {
    Router::new()
        .route("/mcp", post(mcp_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}

/// POST /mcp — receive JSON-RPC and route to the appropriate Orb.
async fn mcp_handler(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let wants_sse = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(',').any(|part| part.trim() == "text/event-stream"))
        .unwrap_or(false);

    match handle_request(&state.manager, request).await {
        Ok(Some(response)) => {
            if wants_sse {
                let body = serde_json::to_string(&response).unwrap_or_default();
                let sse_body = format!("event: message\ndata: {body}\n\n");
                let mut resp = (StatusCode::OK, sse_body).into_response();
                resp.headers_mut().insert(
                    header::CONTENT_TYPE,
                    "text/event-stream; charset=utf-8".parse().unwrap(),
                );
                resp
            } else {
                Json(response).into_response()
            }
        }
        Ok(None) => StatusCode::ACCEPTED.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Internal error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Defense-in-depth gate for a localhost endpoint. Rejects non-loopback Host
/// headers (blunts DNS-rebinding from a malicious web page) and, when a token
/// is configured, requires it via `Authorization: Bearer` or `?token=`.
async fn auth_middleware(
    State(state): State<Arc<GatewayState>>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !host_allowed(host) {
        return Err(reject(
            StatusCode::FORBIDDEN,
            -32002,
            "forbidden: non-loopback host",
        ));
    }

    if let Some(expected) = &state.token {
        let provided = bearer_token(req.headers()).or_else(|| query_token(req.uri()));
        match provided {
            Some(p) if constant_time_eq(p.as_bytes(), expected.as_bytes()) => {}
            _ => return Err(reject(StatusCode::UNAUTHORIZED, -32001, "unauthorized")),
        }
    }

    Ok(next.run(req).await)
}

fn reject(code: StatusCode, jsonrpc_code: i64, message: &str) -> Response {
    (
        code,
        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "error": { "code": jsonrpc_code, "message": message }
        })),
    )
        .into_response()
}

/// Loopback-only Host allowlist: `127.0.0.1`, `localhost`, and the IPv6 `::1`
/// literal, with any port.
fn host_allowed(host: &str) -> bool {
    let h = host.trim();
    if let Some(rest) = h.strip_prefix('[') {
        return rest.split(']').next() == Some("::1");
    }
    let host_only = h.split(':').next().unwrap_or(h);
    matches!(host_only, "127.0.0.1" | "localhost")
}

/// Extract a `Bearer` credential from the `Authorization` header, case-insensitive.
fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let scheme = value.split_once(' ')?;
    if scheme.0.eq_ignore_ascii_case("Bearer") {
        Some(scheme.1.trim().to_string())
    } else {
        None
    }
}

/// Extract a `token` value from the query string (`?token=...`). The token is
/// URL-safe base64 so no percent-decoding is required here.
fn query_token(uri: &Uri) -> Option<String> {
    let query = uri.query()?;
    query.split('&').find_map(|pair| {
        let (key, val) = pair.split_once('=')?;
        (key == "token").then(|| val.to_string())
    })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist_accepts_loopback_only() {
        assert!(host_allowed("127.0.0.1:5599"));
        assert!(host_allowed("127.0.0.1"));
        assert!(host_allowed("localhost:5599"));
        assert!(host_allowed("[::1]:5599"));
        assert!(host_allowed("[::1]"));
        assert!(!host_allowed("192.168.1.10:5599"));
        assert!(!host_allowed("evil.example.com:5599"));
        assert!(!host_allowed(""));
    }

    #[test]
    fn bearer_token_parses_case_insensitively() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), None);
        headers.insert(header::AUTHORIZATION, "Bearer abc123".parse().unwrap());
        assert_eq!(bearer_token(&headers).as_deref(), Some("abc123"));
        headers.insert(header::AUTHORIZATION, "bearer xyz".parse().unwrap());
        assert_eq!(bearer_token(&headers).as_deref(), Some("xyz"));
        headers.insert(header::AUTHORIZATION, "Basic dXNlcjpwYXNz".parse().unwrap());
        assert_eq!(bearer_token(&headers), None);
    }

    #[test]
    fn query_token_extracts_only_token_key() {
        let uri = "http://127.0.0.1:5599/mcp?token=sekret&x=1".parse::<Uri>().unwrap();
        assert_eq!(query_token(&uri).as_deref(), Some("sekret"));
        let uri = "http://127.0.0.1:5599/mcp".parse::<Uri>().unwrap();
        assert_eq!(query_token(&uri), None);
        let uri = "http://127.0.0.1:5599/mcp?other=1".parse::<Uri>().unwrap();
        assert_eq!(query_token(&uri), None);
    }

    #[test]
    fn constant_time_eq_matches_exact_bytes_only() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    // ── auth middleware integration (router-level, no real server) ──────────

    fn test_state(token: Option<&str>) -> Arc<GatewayState> {
        let manager = Arc::new(RuntimeManager::new(GatewayConfig::default(), vec![]));
        Arc::new(GatewayState {
            manager,
            token: token.map(Arc::from),
        })
    }

    fn request(uri: &str, token: Option<&str>) -> Request {
        let mut builder = Request::builder()
            .method("POST")
            .uri(uri)
            // Real HTTP clients always send a Host header; bare oneshot requests
            // do not, and the host allowlist treats an absent Host as non-loopback.
            .header(header::HOST, "127.0.0.1:5599")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        builder
            .body(axum::body::Body::from(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            ))
            .unwrap()
    }

    async fn run(router: Router, req: Request) -> StatusCode {
        use tower::ServiceExt;
        router.oneshot(req).await.unwrap().status()
    }

    #[tokio::test]
    async fn middleware_rejects_missing_token_with_401() {
        let app = build_app(test_state(Some("sekret")));
        let status = run(app, request("http://127.0.0.1:5599/mcp", None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_wrong_token_with_401() {
        let app = build_app(test_state(Some("sekret")));
        let status = run(app, request("http://127.0.0.1:5599/mcp", Some("wrong"))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_non_loopback_host_with_403() {
        let app = build_app(test_state(Some("sekret")));
        let mut req = request("http://127.0.0.1:5599/mcp", Some("sekret"));
        req.headers_mut()
            .insert(header::HOST, "evil.example.com".parse().unwrap());
        let status = run(app, req).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn middleware_accepts_valid_bearer_token() {
        let app = build_app(test_state(Some("sekret")));
        let status = run(app, request("http://127.0.0.1:5599/mcp", Some("sekret"))).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_accepts_valid_query_token() {
        let app = build_app(test_state(Some("sekret")));
        let status = run(app, request("http://127.0.0.1:5599/mcp?token=sekret", None)).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_skips_auth_when_no_token_configured() {
        let app = build_app(test_state(None));
        let status = run(app, request("http://127.0.0.1:5599/mcp", None)).await;
        assert_eq!(status, StatusCode::OK);
    }
}
