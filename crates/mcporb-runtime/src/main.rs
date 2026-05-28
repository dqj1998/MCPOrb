mod api;
mod assets;
#[cfg(feature = "vector-embedder")]
mod embed_startup;
mod mcp_handler;
mod startup;
mod state;
mod web_server;

mod embedded_orb {
    include!(concat!(env!("OUT_DIR"), "/embedded_orb.rs"));
}

use std::path::PathBuf;

use clap::Parser;
use mcporb_runtime_core::format::Capability;
use mcporb_runtime_core::{
    Bm25Index, Chunk, DenseRuntime, Document, FlatVectorIndex, HnswIndex, OrbManifest,
    SearchRuntime, TfIdfIndex, TrigramIndex,
};
use startup::{detect_startup, StartupMode};
use state::OrbState;

fn load_orb_data(
    assets_path: &std::path::Path,
) -> anyhow::Result<(OrbManifest, Vec<Document>, Vec<Chunk>, SearchRuntime)> {
    let manifest_json = std::fs::read(assets_path.join("orb_manifest.json"))?;
    let docs_bytes = std::fs::read(assets_path.join("documents.postcard"))?;
    let chunks_bytes = std::fs::read(assets_path.join("chunks.postcard"))?;
    let index_bytes = std::fs::read(assets_path.join("bm25_index.postcard"))?;
    let tfidf_bytes = read_optional_asset(assets_path.join("tfidf_index.postcard"))?;
    let trigram_bytes = read_optional_asset(assets_path.join("trigram_index.postcard"))?;
    let vector_bytes = read_optional_asset(assets_path.join("vector_store.postcard"))?;
    let hnsw_bytes = read_optional_asset(assets_path.join("hnsw_index.postcard"))?;
    load_orb_data_from_bytes(
        &manifest_json,
        &docs_bytes,
        &chunks_bytes,
        &index_bytes,
        tfidf_bytes.as_deref(),
        trigram_bytes.as_deref(),
        vector_bytes.as_deref(),
        hnsw_bytes.as_deref(),
    )
}

fn load_embedded_orb_data() -> anyhow::Result<(OrbManifest, Vec<Document>, Vec<Chunk>, SearchRuntime)> {
    anyhow::ensure!(embedded_orb::HAS_EMBEDDED_ORB, "no embedded orb assets were compiled into this binary");

    load_orb_data_from_bytes(
        embedded_orb::EMBEDDED_MANIFEST_JSON,
        embedded_orb::EMBEDDED_DOCUMENTS,
        embedded_orb::EMBEDDED_CHUNKS,
        embedded_orb::EMBEDDED_INDEX,
        if embedded_orb::EMBEDDED_TFIDF_INDEX.is_empty() {
            None
        } else {
            Some(embedded_orb::EMBEDDED_TFIDF_INDEX)
        },
        if embedded_orb::EMBEDDED_TRIGRAM_INDEX.is_empty() {
            None
        } else {
            Some(embedded_orb::EMBEDDED_TRIGRAM_INDEX)
        },
        if embedded_orb::EMBEDDED_VECTOR_STORE.is_empty() {
            None
        } else {
            Some(embedded_orb::EMBEDDED_VECTOR_STORE)
        },
        if embedded_orb::EMBEDDED_HNSW_INDEX.is_empty() {
            None
        } else {
            Some(embedded_orb::EMBEDDED_HNSW_INDEX)
        },
    )
}

fn load_orb_data_from_bytes(
    manifest_json: &[u8],
    docs_bytes: &[u8],
    chunks_bytes: &[u8],
    index_bytes: &[u8],
    tfidf_bytes: Option<&[u8]>,
    trigram_bytes: Option<&[u8]>,
    vector_bytes: Option<&[u8]>,
    hnsw_bytes: Option<&[u8]>,
) -> anyhow::Result<(OrbManifest, Vec<Document>, Vec<Chunk>, SearchRuntime)> {
    let manifest: OrbManifest = serde_json::from_slice(manifest_json)?;
    let documents: Vec<Document> = postcard::from_bytes(docs_bytes)?;
    let chunks: Vec<Chunk> = postcard::from_bytes(chunks_bytes)?;
    let index: Bm25Index = postcard::from_bytes(index_bytes)?;
    let tfidf = load_optional_index::<TfIdfIndex>(&manifest, Capability::TfIdf, tfidf_bytes)?;
    let trigram = load_optional_index::<TrigramIndex>(&manifest, Capability::Trigram, trigram_bytes)?;
    let vector = load_optional_index::<FlatVectorIndex>(&manifest, Capability::FlatVector, vector_bytes)?;
    let hnsw = load_optional_index::<HnswIndex>(&manifest, Capability::Hnsw, hnsw_bytes)?;
    let search = SearchRuntime {
        bm25: index,
        tfidf,
        trigram,
        dense: DenseRuntime::from_assets(vector, hnsw)?,
        dense_tier: manifest.selected_retrieval_plan.clone(),
    };
    Ok((manifest, documents, chunks, search))
}

fn demo_manifest() -> (OrbManifest, Vec<Document>, Vec<Chunk>, SearchRuntime) {
    use mcporb_runtime_core::format::{Capability, RetrievalPlanKind};
    let manifest = OrbManifest {
        name: "demo-orb".to_string(),
        version: "0.1.0".to_string(),
        description: "Demo Orb — no assets loaded".to_string(),
        orb_format_version: "0.2".to_string(),
        mcp_protocol_version: "2024-11-05".to_string(),
        build_time: "unknown".to_string(),
        source_documents: vec![],
        chunk_count: 0,
        index_format_version: "0.2".to_string(),
        binary_size_target_mb: 15,
        selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
        enabled_capabilities: vec![Capability::Bm25],
        embedding_dim: None,
        embedding_model: None,
        embedding_model_tar_sha256: None,
        trigram_min_df: None,
        planning_rationale: vec![serde_json::json!("Demo mode — no assets loaded.")],
    };
    (
        manifest,
        vec![],
        vec![],
        SearchRuntime {
            bm25: Bm25Index::default(),
            tfidf: None,
            trigram: None,
            dense: DenseRuntime::None,
            dense_tier: RetrievalPlanKind::Bm25Only,
        },
    )
}

fn detect_orb_binary_path(config: &startup::StartupConfig) -> Option<String> {
    if config.assets_path.is_some() {
        return None;
    }

    if embedded_orb::HAS_EMBEDDED_ORB {
        return std::env::current_exe()
            .ok()
            .map(|path| path.canonicalize().unwrap_or(path))
            .map(|path| path.display().to_string());
    }

    // Sidecar mode: look for <exe_path>.data/ directory
    let exe = std::env::current_exe().ok()?;
    let exe_str = exe.to_string_lossy();
    let data_dir = PathBuf::from(format!("{}.data", exe_str));
    if data_dir.join("orb_manifest.json").exists() {
        Some(exe.canonicalize().unwrap_or(exe).display().to_string())
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    let args = startup::OrbArgs::parse();
    let mut config = detect_startup(&args);

    tracing::info!(mode = ?config.mode, "MCPOrb runtime starting");

    // Sidecar auto-detection: if no --assets and no embedded orb, look for
    // <exe_path>.data/ directory next to the binary.
    if config.assets_path.is_none() && !embedded_orb::HAS_EMBEDDED_ORB {
        if let Ok(exe) = std::env::current_exe() {
            let data_dir = PathBuf::from(format!("{}.data", exe.to_string_lossy()));
            if data_dir.join("orb_manifest.json").exists() {
                config = startup::StartupConfig {
                    assets_path: Some(data_dir),
                    ..config
                };
            }
        }
    }

    let (manifest, documents, chunks, search) = if let Some(ref p) = config.assets_path {
        load_orb_data(p)?
    } else if embedded_orb::HAS_EMBEDDED_ORB {
        load_embedded_orb_data()?
    } else {
        demo_manifest()
    };

    let mode_str = format!("{:?}", config.mode);
    let orb_binary_path = detect_orb_binary_path(&config);
    #[cfg(feature = "vector-embedder")]
    let (model_manager, embedder_slot) = embed_startup::prepare(&manifest)?;

    match config.mode {
        StartupMode::StdioOnly => {
            let state = OrbState::new(
                manifest, documents, chunks, search,
                #[cfg(feature = "vector-embedder")] model_manager,
                #[cfg(feature = "vector-embedder")] embedder_slot,
                mode_str, orb_binary_path, None,
            );
            mcp_handler::run_stdio_loop(state).await?;
        }
        StartupMode::GuiOnly => {
            let token = web_server::generate_token();
            let state = OrbState::new(
                manifest, documents, chunks, search,
                #[cfg(feature = "vector-embedder")] model_manager,
                #[cfg(feature = "vector-embedder")] embedder_slot,
                mode_str, orb_binary_path, None,
            );
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
            let state = OrbState::new(
                manifest, documents, chunks, search,
                #[cfg(feature = "vector-embedder")] model_manager,
                #[cfg(feature = "vector-embedder")] embedder_slot,
                mode_str, orb_binary_path, None,
            );
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

fn read_optional_asset(path: std::path::PathBuf) -> anyhow::Result<Option<Vec<u8>>> {
    if path.exists() {
        Ok(Some(std::fs::read(path)?))
    } else {
        Ok(None)
    }
}

fn load_optional_index<T>(
    manifest: &OrbManifest,
    capability: Capability,
    bytes: Option<&[u8]>,
) -> anyhow::Result<Option<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let capability_enabled = manifest
        .enabled_capabilities
        .iter()
        .any(|value| *value == capability);

    match (capability_enabled, bytes) {
        (true, Some(bytes)) => Ok(Some(postcard::from_bytes(bytes)?)),
        (true, None) => anyhow::bail!("missing asset for enabled capability {:?}", capability),
        (false, Some(bytes)) => Ok(Some(postcard::from_bytes(bytes)?)),
        (false, None) => Ok(None),
    }
}
