//! MCPOrb Gateway — STDIO transport.
//!
//! Reads JSON-RPC messages from stdin, routes them to the appropriate Orb
//! child process, and writes responses to stdout.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use mcporb_gateway_core::handler::handle_request;
use mcporb_gateway_core::registry_reader::{discover_orbs, GatewayConfig};
use mcporb_gateway_core::runtime_manager::RuntimeManager;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing;

#[derive(Parser, Debug)]
#[command(
    name = "mcporb-gateway-stdio",
    about = "MCPOrb Gateway — single STDIO MCP endpoint routing to multiple Orbs"
)]
struct Args {
    /// Registry directory (default: ~/Library/Application Support/MCPOrb/Runtime)
    #[arg(long)]
    registry_dir: Option<PathBuf>,

    /// Path to mcporb-runtime binary (default: auto-detect next to gateway)
    #[arg(long)]
    runtime_binary: Option<PathBuf>,

    /// Idle timeout in seconds before recycling an Orb process (default: 300)
    #[arg(long, default_value = "300")]
    idle_timeout: u64,

    /// Check interval in seconds for idle reaper (default: 30)
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
        idle_timeout = %config.idle_timeout_secs,
        "MCPOrb Gateway (STDIO) starting"
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

    // Response channel: handler tasks → writer task → stdout.
    // Using a channel decouples request handling from stdout writes so that
    // a slow or long-running tool call never blocks reading the next request.
    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel::<String>(64);

    // Writer task: serialises all response writes to stdout.
    let writer = tokio::spawn(async move {
        let mut out = tokio::io::stdout();
        while let Some(line) = resp_rx.recv().await {
            if out.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if out.write_all(b"\n").await.is_err() {
                break;
            }
            if out.flush().await.is_err() {
                break;
            }
        }
    });

    // STDIO reader loop: parse each incoming line and spawn a handler task.
    // The loop keeps reading stdin concurrently with in-flight tool calls so
    // that ping / tools/list / initialize always get an immediate response.
    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!(line = %line, error = %e, "Failed to parse JSON-RPC");
                let error_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                let _ = resp_tx
                    .send(serde_json::to_string(&error_resp).unwrap_or_default())
                    .await;
                continue;
            }
        };

        let manager_clone = manager.clone();
        let resp_tx_clone = resp_tx.clone();
        tokio::spawn(async move {
            match handle_request(&manager_clone, request).await {
                Ok(Some(response)) => {
                    let s = serde_json::to_string(&response).unwrap_or_default();
                    let _ = resp_tx_clone.send(s).await;
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::error!(error = %e, "Internal error");
                    let error_resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32603, "message": format!("Internal error: {e}") }
                    });
                    let _ = resp_tx_clone
                        .send(serde_json::to_string(&error_resp).unwrap_or_default())
                        .await;
                }
            }
        });
    }

    // Stdin closed — signal the writer that no more responses are coming.
    drop(resp_tx);
    writer.await.ok();

    tracing::info!("STDIO loop ended, shutting down");
    manager.shutdown().await;
    Ok(())
}
