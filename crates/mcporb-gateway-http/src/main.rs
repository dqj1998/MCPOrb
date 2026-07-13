//! MCPOrb Gateway — HTTP transport.
//!
//! Serves a single HTTP MCP endpoint at `POST /mcp` that routes JSON-RPC
//! requests to the appropriate Orb child process.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
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

    let app = Router::new()
        .route("/mcp", post(mcp_handler))
        .with_state(manager);

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind address: {e}"))?;

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(addr = %addr, "HTTP server listening");

    axum::serve(listener, app).await?;

    Ok(())
}

/// POST /mcp — receive JSON-RPC and route to the appropriate Orb.
async fn mcp_handler(
    State(manager): State<Arc<RuntimeManager>>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let wants_sse = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(',').any(|part| part.trim() == "text/event-stream"))
        .unwrap_or(false);

    match handle_request(&manager, request).await {
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
