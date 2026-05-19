mod api;
mod assets;
mod mcp_handler;
mod startup;
mod state;
mod web_server;

mod embedded_orb {
    include!(concat!(env!("OUT_DIR"), "/embedded_orb.rs"));
}

use clap::Parser;
use mcporb_runtime_core::{Bm25Index, Chunk, Document, OrbManifest};
use startup::{detect_startup, StartupMode};
use state::OrbState;

fn load_orb_data(assets_path: &std::path::Path) -> anyhow::Result<(OrbManifest, Vec<Document>, Vec<Chunk>, Bm25Index)> {
    let manifest_json = std::fs::read(assets_path.join("orb_manifest.json"))?;
    let docs_bytes = std::fs::read(assets_path.join("documents.postcard"))?;
    let chunks_bytes = std::fs::read(assets_path.join("chunks.postcard"))?;
    let index_bytes = std::fs::read(assets_path.join("bm25_index.postcard"))?;
    load_orb_data_from_bytes(&manifest_json, &docs_bytes, &chunks_bytes, &index_bytes)
}

fn load_embedded_orb_data() -> anyhow::Result<(OrbManifest, Vec<Document>, Vec<Chunk>, Bm25Index)> {
    anyhow::ensure!(embedded_orb::HAS_EMBEDDED_ORB, "no embedded orb assets were compiled into this binary");

    load_orb_data_from_bytes(
        embedded_orb::EMBEDDED_MANIFEST_JSON,
        embedded_orb::EMBEDDED_DOCUMENTS,
        embedded_orb::EMBEDDED_CHUNKS,
        embedded_orb::EMBEDDED_INDEX,
    )
}

fn load_orb_data_from_bytes(
    manifest_json: &[u8],
    docs_bytes: &[u8],
    chunks_bytes: &[u8],
    index_bytes: &[u8],
) -> anyhow::Result<(OrbManifest, Vec<Document>, Vec<Chunk>, Bm25Index)> {
    let manifest: OrbManifest = serde_json::from_slice(manifest_json)?;
    let documents: Vec<Document> = postcard::from_bytes(docs_bytes)?;
    let chunks: Vec<Chunk> = postcard::from_bytes(chunks_bytes)?;
    let index: Bm25Index = postcard::from_bytes(index_bytes)?;
    Ok((manifest, documents, chunks, index))
}

fn demo_manifest() -> (OrbManifest, Vec<Document>, Vec<Chunk>, Bm25Index) {
    let manifest = OrbManifest {
        name: "demo-orb".to_string(),
        version: "0.1.0".to_string(),
        description: "Demo Orb — no assets loaded".to_string(),
        orb_format_version: "0.1".to_string(),
        mcp_protocol_version: "2024-11-05".to_string(),
        build_time: "unknown".to_string(),
        source_documents: vec![],
        chunk_count: 0,
        index_format_version: "0.1".to_string(),
        binary_size_target_mb: 15,
    };
    (manifest, vec![], vec![], Bm25Index::default())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    let args = startup::OrbArgs::parse();
    let config = detect_startup(&args);

    tracing::info!(mode = ?config.mode, "MCPOrb runtime starting");

    let (manifest, documents, chunks, index) = if let Some(ref p) = config.assets_path {
        load_orb_data(p)?
    } else if embedded_orb::HAS_EMBEDDED_ORB {
        load_embedded_orb_data()?
    } else {
        demo_manifest()
    };

    let mode_str = format!("{:?}", config.mode);

    match config.mode {
        StartupMode::StdioOnly => {
            let state = OrbState::new(manifest, documents, chunks, index, mode_str, None);
            mcp_handler::run_stdio_loop(state).await?;
        }
        StartupMode::GuiOnly => {
            let token = web_server::generate_token();
            let state = OrbState::new(manifest, documents, chunks, index, mode_str, None);
            let (addr, server_handle) = web_server::serve(state.clone(), config.port, &token).await?;
            let url = format!("http://127.0.0.1:{}/{}/", addr.port(), token);
            *state.gui_url.write().await = Some(url.clone());
            let tmp = std::env::temp_dir().join("mcporb");
            let _ = std::fs::create_dir_all(&tmp);
            let _ = std::fs::write(tmp.join("orb.url"), &url);
            eprintln!("MCPOrb Web UI: {url}");
            tracing::info!(%url, "Web UI available");
            if config.auto_open { let _ = webbrowser::open(&url); }
            server_handle.await?;
        }
        StartupMode::StdioGui => {
            let token = web_server::generate_token();
            let state = OrbState::new(manifest, documents, chunks, index, mode_str, None);
            let (addr, server_handle) = web_server::serve(state.clone(), config.port, &token).await?;
            let url = format!("http://127.0.0.1:{}/{}/", addr.port(), token);
            *state.gui_url.write().await = Some(url.clone());
            let tmp = std::env::temp_dir().join("mcporb");
            let _ = std::fs::create_dir_all(&tmp);
            let _ = std::fs::write(tmp.join("orb.url"), &url);
            eprintln!("MCPOrb Web UI: {url}");
            tracing::info!(%url, "Web UI available (stdio+gui mode)");
            if config.auto_open { let _ = webbrowser::open(&url); }
            let stdio_state = state.clone();
            let stdio_handle = tokio::spawn(async move {
                if let Err(e) = mcp_handler::run_stdio_loop(stdio_state).await {
                    tracing::error!("MCP stdio error: {e}");
                }
            });
            tokio::select! {
                _ = server_handle => tracing::info!("HTTP server stopped"),
                _ = stdio_handle => tracing::info!("MCP stdio loop ended"),
            }
        }
    }

    let _ = std::fs::remove_file(std::env::temp_dir().join("mcporb").join("orb.url"));
    Ok(())
}
