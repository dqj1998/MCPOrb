mod api;
mod assets;
#[cfg(feature = "vector-embedder")]
mod embed_startup;
mod device_unlock;
mod encrypted_assets;
mod mcp_handler;
mod security;
mod startup;
mod state;
mod web_server;

mod embedded_orb {
    include!(concat!(env!("OUT_DIR"), "/embedded_orb.rs"));
}

use std::io::{Cursor, Read, Seek, SeekFrom};

use clap::Parser;
use mcporb_runtime_core::format::Capability;
use mcporb_runtime_core::{
    Bm25Index, Chunk, DenseRuntime, Document, FlatVectorIndex, HnswIndex, OrbManifest,
    SearchRuntime, TfIdfIndex, TrigramIndex,
};
use security::SecurityConfig;
use startup::{detect_startup, StartupMode};
use state::{LoadedAssets, LoadedKnowledge, LoadedOrb, OrbState};

const APPENDED_BUNDLE_MAGIC: &[u8; 16] = b"MCPORB_BUNDLE_V1";
const APPENDED_BUNDLE_TRAILER_SIZE: u64 = 32;

#[derive(Debug, Clone, Copy)]
struct AppendedBundleFooter {
    offset: u64,
    length: u64,
}

fn load_orb_data(assets_path: &std::path::Path) -> anyhow::Result<LoadedOrb> {
    // Dev/test convenience: an assets dir may carry an optional orb_security.json
    // alongside the plaintext knowledge files (plan §4.2).
    let security = parse_security(read_optional_asset(assets_path.join("orb_security.json"))?)?;
    let manifest_json = std::fs::read(assets_path.join("orb_manifest.json"))?;
    let docs_bytes = std::fs::read(assets_path.join("documents.postcard"))?;
    let chunks_bytes = std::fs::read(assets_path.join("chunks.postcard"))?;
    let index_bytes = std::fs::read(assets_path.join("bm25_index.postcard"))?;
    let tfidf_bytes = read_optional_asset(assets_path.join("tfidf_index.postcard"))?;
    let trigram_bytes = read_optional_asset(assets_path.join("trigram_index.postcard"))?;
    let vector_bytes = read_optional_asset(assets_path.join("vector_store.postcard"))?;
    let hnsw_bytes = read_optional_asset(assets_path.join("hnsw_index.postcard"))?;
    let knowledge = load_orb_data_from_bytes(
        &manifest_json,
        &docs_bytes,
        &chunks_bytes,
        &index_bytes,
        tfidf_bytes.as_deref(),
        trigram_bytes.as_deref(),
        vector_bytes.as_deref(),
        hnsw_bytes.as_deref(),
    )?;
    Ok(LoadedOrb {
        security,
        assets: LoadedAssets::Plain(knowledge),
    })
}

fn load_embedded_orb_data() -> anyhow::Result<LoadedOrb> {
    anyhow::ensure!(
        embedded_orb::HAS_EMBEDDED_ORB,
        "no embedded orb assets were compiled into this binary"
    );

    let knowledge = load_orb_data_from_bytes(
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
    )?;
    // Embedded Orbs carry no security config today; default to disabled.
    Ok(LoadedOrb {
        security: SecurityConfig::disabled(),
        assets: LoadedAssets::Plain(knowledge),
    })
}

fn load_appended_orb_data(binary_path: &std::path::Path) -> anyhow::Result<LoadedOrb> {
    let footer = read_appended_bundle_footer(binary_path)?.ok_or_else(|| {
        anyhow::anyhow!("no appended orb bundle found in {}", binary_path.display())
    })?;
    let bundle_bytes = read_appended_bundle_bytes(binary_path, footer)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bundle_bytes))?;
    load_orb_from_archive(&mut archive)
}

fn load_sidecar_orb_data(binary_path: &std::path::Path) -> anyhow::Result<LoadedOrb> {
    let bundle_bytes = std::fs::read(sidecar_bundle_path(binary_path))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bundle_bytes))?;
    load_orb_from_archive(&mut archive)
}

/// Shared bundle reader for appended and sidecar `.orb` layouts. Reads the
/// optional `orb_security.json` first (plan §4.2), then the plaintext knowledge
/// assets. (Asset-encryption — skipping the plaintext reads in favor of
/// `orb_assets.enc` — is wired in Phase 4.)
fn load_orb_from_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> anyhow::Result<LoadedOrb> {
    let security = parse_security(read_optional_bundle_asset(archive, "orb_security.json")?)?;

    // Encrypted Orb (plan §4.2): the plaintext assets are absent — only
    // `orb_assets.enc` is present. Hold the ciphertext; it is decrypted on the
    // first successful unlock. `asset_encryption` is `Some` only when enabled.
    if security.asset_encryption.is_some() {
        let blob = read_bundle_asset(archive, "orb_assets.enc")?;
        return Ok(LoadedOrb {
            security,
            assets: LoadedAssets::Encrypted(blob),
        });
    }

    let knowledge = read_knowledge_from_archive(archive)?;
    Ok(LoadedOrb {
        security,
        assets: LoadedAssets::Plain(knowledge),
    })
}

/// Read the plaintext knowledge assets (manifest + postcard files) from a zip
/// archive and build a [`LoadedKnowledge`]. Shared by the plaintext bundle path
/// and the post-decryption path ([`build_knowledge_from_asset_zip`]).
fn read_knowledge_from_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> anyhow::Result<LoadedKnowledge> {
    let manifest_json = read_bundle_asset(archive, "orb_manifest.json")?;
    let docs_bytes = read_bundle_asset(archive, "documents.postcard")?;
    let chunks_bytes = read_bundle_asset(archive, "chunks.postcard")?;
    let index_bytes = read_bundle_asset(archive, "bm25_index.postcard")?;
    let tfidf_bytes = read_optional_bundle_asset(archive, "tfidf_index.postcard")?;
    let trigram_bytes = read_optional_bundle_asset(archive, "trigram_index.postcard")?;
    let vector_bytes = read_optional_bundle_asset(archive, "vector_store.postcard")?;
    let hnsw_bytes = read_optional_bundle_asset(archive, "hnsw_index.postcard")?;

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

/// Parse a decrypted asset zip (the plaintext bytes recovered from
/// `orb_assets.enc`) into a [`LoadedKnowledge`]. Used on unlock (plan §4.4).
fn build_knowledge_from_asset_zip(zip_bytes: &[u8]) -> anyhow::Result<LoadedKnowledge> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))?;
    read_knowledge_from_archive(&mut archive)
}

/// Full unlock from a user-entered password (Web `auth/unlock`, MCP `unlock_orb`,
/// or `--unlock` priming). Runs Argon2, then the shared [`finish_unlock`].
/// Idempotent: a no-op `Ok` if already unlocked / no password required.
pub(crate) fn perform_unlock(state: &OrbState, password: &str) -> Result<(), security::AuthError> {
    if state.security.is_unlocked() || !state.security.password_required() {
        return Ok(());
    }
    let keys = state.security.derive_keys(password)?; // Argon2 + HKDF
    finish_unlock(state, keys, /* persist = */ true)
}

/// Complete an unlock from already-derived keys (from a password or a recalled
/// keychain `master_key`). Verifies (password-only) or decrypts + loads
/// (encrypted), marks the process unlocked, and — when `persist` and the Orb is
/// `remember_on_this_device` — stores `master_key` in the OS keychain so future
/// launches auto-unlock.
fn finish_unlock(
    state: &OrbState,
    keys: security::DerivedKeys,
    persist: bool,
) -> Result<(), security::AuthError> {
    use security::AuthError;

    // Asset encryption is enabled iff the config carries an encryption block.
    let enc = state.security.config().asset_encryption.clone();
    match enc {
        None => {
            // Password-only: verifier check.
            if !state.security.verify_keys(&keys) {
                state.security.record_failure();
                return Err(AuthError::InvalidPassword);
            }
            state.security.mark_unlocked();
        }
        Some(enc) => {
            let blob = state
                .encrypted_blob_clone()
                .ok_or_else(|| AuthError::Crypto("encrypted Orb has no asset payload".into()))?;
            match encrypted_assets::decrypt_asset_blob(&enc, &keys.asset_key, &blob) {
                Ok(zip_bytes) => {
                    let knowledge = build_knowledge_from_asset_zip(&zip_bytes).map_err(|e| {
                        AuthError::Crypto(format!("decrypted asset parse failed: {e}"))
                    })?;
                    // Kick off the embedder now that the manifest is available (it
                    // was inside the encrypted payload, unavailable at startup).
                    #[cfg(feature = "vector-embedder")]
                    embed_startup::start_for_manifest(
                        state.model_manager.clone(),
                        state.embedder_slot.clone(),
                        &knowledge.manifest,
                    );
                    state.set_knowledge(knowledge);
                    state.clear_encrypted_blob();
                    state.security.mark_unlocked();
                }
                Err(e) => {
                    // Wrong password (AEAD tag fail) or corrupt payload — rate-limit.
                    state.security.record_failure();
                    return Err(e);
                }
            }
        }
    }

    // Success. Persist for remember-on-device (best-effort; never blocks unlock).
    if persist {
        if let Some(pc) = state.security.config().password.as_ref() {
            if pc.unlock_persistence == security::UnlockPersistence::RememberOnThisDevice {
                if let Err(e) = device_unlock::remember(&pc.orb_identity, &keys.master_key) {
                    tracing::warn!("could not remember unlock on this device: {e}");
                }
            }
        }
    }
    Ok(())
}

/// At startup, for a `remember_on_this_device` Orb, try to unlock from a key
/// stored in the OS keychain — no password prompt. A stale/invalid stored key
/// is dropped so the normal password flow takes over. For `every_launch` Orbs,
/// proactively clear any leftover entry (e.g. after a policy change).
fn try_auto_unlock(state: &OrbState) {
    let Some(pc) = state.security.config().password.as_ref() else {
        return;
    };
    if pc.unlock_persistence != security::UnlockPersistence::RememberOnThisDevice {
        device_unlock::forget(&pc.orb_identity);
        return;
    }
    let orb_identity = pc.orb_identity.clone();
    let Some(master_key) = device_unlock::recall(&orb_identity) else {
        return;
    };
    let keys = security::derive_from_master(master_key);
    match finish_unlock(state, keys, /* persist = */ false) {
        Ok(()) => tracing::info!("unlocked from remembered device credential"),
        Err(_) => {
            tracing::warn!("remembered device credential invalid; forgetting it");
            device_unlock::forget(&orb_identity);
        }
    }
}

/// `--unlock`: prompt for the password once (hidden) and, on success, remember
/// it on this device so subsequent (e.g. stdio MCP) launches auto-unlock without
/// the password ever entering the LLM conversation (plan §3.2 fallback B).
fn prime_device_unlock(state: &OrbState) -> anyhow::Result<()> {
    let persistence = state
        .security
        .config()
        .password
        .as_ref()
        .map(|p| p.unlock_persistence);
    match persistence {
        None => anyhow::bail!("this Orb is not password-protected — nothing to unlock"),
        Some(security::UnlockPersistence::EveryLaunch) => anyhow::bail!(
            "this Orb uses 'require password every launch' — there is nothing to remember; \
             package with --remember-unlock to enable device unlock"
        ),
        Some(security::UnlockPersistence::RememberOnThisDevice) => {}
    }

    let password = rpassword::prompt_password("Orb password: ")
        .map_err(|e| anyhow::anyhow!("could not read password: {e}"))?;
    match perform_unlock(state, &password) {
        Ok(()) => {
            println!("✅ Unlock remembered on this device. Future launches won't prompt.");
            Ok(())
        }
        Err(_) => anyhow::bail!("Invalid password"),
    }
}

fn try_load_self_bundle() -> anyhow::Result<Option<LoadedOrb>> {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(_) => return Ok(None),
    };

    if sidecar_bundle_path(&exe).is_file() {
        return load_sidecar_orb_data(&exe).map(Some);
    }

    if read_appended_bundle_footer(&exe)?.is_none() {
        return Ok(None);
    }

    load_appended_orb_data(&exe).map(Some)
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
) -> anyhow::Result<LoadedKnowledge> {
    let manifest: OrbManifest = serde_json::from_slice(manifest_json)?;
    let documents: Vec<Document> = postcard::from_bytes(docs_bytes)?;
    let chunks: Vec<Chunk> = postcard::from_bytes(chunks_bytes)?;
    let index: Bm25Index = postcard::from_bytes(index_bytes)?;
    let tfidf = load_optional_index::<TfIdfIndex>(&manifest, Capability::TfIdf, tfidf_bytes)?;
    let trigram =
        load_optional_index::<TrigramIndex>(&manifest, Capability::Trigram, trigram_bytes)?;
    let vector =
        load_optional_index::<FlatVectorIndex>(&manifest, Capability::FlatVector, vector_bytes)?;
    let hnsw = load_optional_index::<HnswIndex>(&manifest, Capability::Hnsw, hnsw_bytes)?;
    let search = SearchRuntime {
        bm25: index,
        tfidf,
        trigram,
        dense: DenseRuntime::from_assets(vector, hnsw)?,
        dense_tier: manifest.selected_retrieval_plan.clone(),
    };
    Ok(LoadedKnowledge {
        manifest,
        documents,
        chunks,
        search,
    })
}

/// Parse an optional `orb_security.json` blob into a [`SecurityConfig`].
/// Absent file → disabled (no password, no encryption).
fn parse_security(bytes: Option<Vec<u8>>) -> anyhow::Result<SecurityConfig> {
    match bytes {
        Some(b) => SecurityConfig::from_bundle_json(&b)
            .map_err(|e| anyhow::anyhow!("invalid orb_security.json: {e}")),
        None => Ok(SecurityConfig::disabled()),
    }
}

fn demo_manifest() -> LoadedOrb {
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
    LoadedOrb {
        security: SecurityConfig::disabled(),
        assets: LoadedAssets::Plain(LoadedKnowledge {
            manifest,
            documents: vec![],
            chunks: vec![],
            search: SearchRuntime {
                bm25: Bm25Index::default(),
                tfidf: None,
                trigram: None,
                dense: DenseRuntime::None,
                dense_tier: RetrievalPlanKind::Bm25Only,
            },
        }),
    }
}

fn read_appended_bundle_footer(
    binary_path: &std::path::Path,
) -> anyhow::Result<Option<AppendedBundleFooter>> {
    let mut file = std::fs::File::open(binary_path)?;
    let file_len = file.metadata()?.len();
    if file_len < APPENDED_BUNDLE_TRAILER_SIZE {
        return Ok(None);
    }

    file.seek(SeekFrom::End(-(APPENDED_BUNDLE_TRAILER_SIZE as i64)))?;
    let mut trailer = [0u8; APPENDED_BUNDLE_TRAILER_SIZE as usize];
    file.read_exact(&mut trailer)?;

    if &trailer[..APPENDED_BUNDLE_MAGIC.len()] != APPENDED_BUNDLE_MAGIC {
        return Ok(None);
    }

    let offset = u64::from_le_bytes(
        trailer[APPENDED_BUNDLE_MAGIC.len()..APPENDED_BUNDLE_MAGIC.len() + 8]
            .try_into()
            .unwrap(),
    );
    let length = u64::from_le_bytes(
        trailer[APPENDED_BUNDLE_MAGIC.len() + 8..APPENDED_BUNDLE_MAGIC.len() + 16]
            .try_into()
            .unwrap(),
    );

    anyhow::ensure!(
        offset <= file_len,
        "invalid appended orb bundle offset {} for {}",
        offset,
        binary_path.display()
    );
    anyhow::ensure!(
        length <= file_len.saturating_sub(APPENDED_BUNDLE_TRAILER_SIZE),
        "invalid appended orb bundle length {} for {}",
        length,
        binary_path.display()
    );
    anyhow::ensure!(
        offset + length + APPENDED_BUNDLE_TRAILER_SIZE == file_len,
        "invalid appended orb bundle trailer for {}",
        binary_path.display()
    );

    Ok(Some(AppendedBundleFooter { offset, length }))
}

fn read_appended_bundle_bytes(
    binary_path: &std::path::Path,
    footer: AppendedBundleFooter,
) -> anyhow::Result<Vec<u8>> {
    let mut file = std::fs::File::open(binary_path)?;
    file.seek(SeekFrom::Start(footer.offset))?;

    let bundle_len = usize::try_from(footer.length)
        .map_err(|_| anyhow::anyhow!("appended orb bundle too large to load on this platform"))?;
    let mut bundle = vec![0u8; bundle_len];
    file.read_exact(&mut bundle)?;
    Ok(bundle)
}

fn read_bundle_asset<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> anyhow::Result<Vec<u8>> {
    let mut file = archive.by_name(name)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_optional_bundle_asset<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> anyhow::Result<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn sidecar_bundle_path(binary_path: &std::path::Path) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}.data", binary_path.display())).join("orb-assets.zip")
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

    let exe = std::env::current_exe().ok()?;
    if sidecar_bundle_path(&exe).is_file()
        || read_appended_bundle_footer(&exe).ok().flatten().is_some()
    {
        return Some(exe.canonicalize().unwrap_or(exe).display().to_string());
    }

    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args = startup::OrbArgs::parse();
    let config = detect_startup(&args);

    tracing::info!(mode = ?config.mode, "MCPOrb runtime starting");

    let loaded = if let Some(ref p) = config.assets_path {
        load_orb_data(p)?
    } else if embedded_orb::HAS_EMBEDDED_ORB {
        load_embedded_orb_data()?
    } else if let Some(data) = try_load_self_bundle()? {
        data
    } else {
        demo_manifest()
    };
    let LoadedOrb { security, assets } = loaded;

    if security.password.is_some() {
        tracing::info!("Password protection enabled for this Orb");
    }

    let mode_str = format!("{:?}", config.mode);
    let orb_binary_path = detect_orb_binary_path(&config);
    // Encrypted Orbs have no manifest until unlock, so the embedder cannot be
    // prepared at startup — `perform_unlock` starts it once the manifest is
    // decrypted. Plaintext Orbs prepare it now as before.
    #[cfg(feature = "vector-embedder")]
    let (model_manager, embedder_slot) = match &assets {
        LoadedAssets::Plain(k) => embed_startup::prepare(&k.manifest)?,
        LoadedAssets::Encrypted(_) => embed_startup::prepare_empty()?,
    };

    let state = OrbState::new(
        security,
        assets,
        #[cfg(feature = "vector-embedder")]
        model_manager,
        #[cfg(feature = "vector-embedder")]
        embedder_slot,
        mode_str,
        orb_binary_path,
        None,
    );

    // `--unlock`: prompt once, remember on this device, then exit (no server).
    if args.unlock {
        return prime_device_unlock(&state);
    }

    // For remember-on-device Orbs, try to unlock from the keychain before
    // serving so the user isn't prompted again on this machine (plan §2.5).
    try_auto_unlock(&state);

    match config.mode {
        StartupMode::StdioOnly => {
            mcp_handler::run_stdio_loop(state).await?;
        }
        StartupMode::GuiOnly => {
            let token = web_server::generate_token();
            let (addr, server_handle) =
                web_server::serve(state.clone(), config.port, &token).await?;
            let url = format!("http://127.0.0.1:{}/{}/", addr.port(), token);
            *state.gui_url.write().await = Some(url.clone());
            let tmp = std::env::temp_dir().join("mcporb");
            let _ = std::fs::create_dir_all(&tmp);
            let _ = std::fs::write(tmp.join("orb.url"), &url);
            eprintln!("MCPOrb Web UI: {url}");
            tracing::info!(%url, "Web UI available");
            if config.auto_open {
                let _ = webbrowser::open(&url);
            }
            server_handle.await?;
        }
        StartupMode::AllGui => {
            let token = web_server::generate_token();
            let (addr, server_handle) =
                web_server::serve(state.clone(), config.port, &token).await?;
            let url = format!("http://127.0.0.1:{}/{}/", addr.port(), token);
            *state.gui_url.write().await = Some(url.clone());
            let tmp = std::env::temp_dir().join("mcporb");
            let _ = std::fs::create_dir_all(&tmp);
            let _ = std::fs::write(tmp.join("orb.url"), &url);
            eprintln!("MCPOrb Web UI: {url}");
            tracing::info!(%url, "Web UI available (all-gui mode)");
            if config.auto_open {
                let _ = webbrowser::open(&url);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    use mcporb_runtime_core::format::RetrievalPlanKind;

    fn append_bundle_footer(mut binary: Vec<u8>, bundle_bytes: &[u8]) -> Vec<u8> {
        let offset = binary.len() as u64;
        let length = bundle_bytes.len() as u64;
        binary.extend_from_slice(bundle_bytes);
        binary.extend_from_slice(APPENDED_BUNDLE_MAGIC);
        binary.extend_from_slice(&offset.to_le_bytes());
        binary.extend_from_slice(&length.to_le_bytes());
        binary
    }

    fn build_test_bundle() -> Vec<u8> {
        let manifest = OrbManifest {
            name: "test-orb".to_string(),
            version: "0.1.0".to_string(),
            description: "single file test".to_string(),
            orb_format_version: "0.2".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            build_time: "2026-06-01T00:00:00Z".to_string(),
            source_documents: vec!["doc.pdf".to_string()],
            chunk_count: 1,
            index_format_version: "0.2".to_string(),
            binary_size_target_mb: 20,
            selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
            enabled_capabilities: vec![Capability::Bm25],
            embedding_dim: None,
            embedding_model: None,
            embedding_model_tar_sha256: None,
            trigram_min_df: None,
            planning_rationale: vec![],
        };
        let documents = vec![Document {
            id: 0,
            title: "Doc".to_string(),
            source_path: "doc.pdf".to_string(),
            page_count: Some(1),
            sections: vec![],
        }];
        let chunks = vec![Chunk {
            id: 0,
            document_id: 0,
            section_id: None,
            page: Some(1),
            text: "hello orb".to_string(),
            token_count: 2,
        }];
        let index = Bm25Index::default();

        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("orb_manifest.json", opts).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        zip.start_file("documents.postcard", opts).unwrap();
        zip.write_all(&postcard::to_allocvec(&documents).unwrap())
            .unwrap();
        zip.start_file("chunks.postcard", opts).unwrap();
        zip.write_all(&postcard::to_allocvec(&chunks).unwrap())
            .unwrap();
        zip.start_file("bm25_index.postcard", opts).unwrap();
        zip.write_all(&postcard::to_allocvec(&index).unwrap())
            .unwrap();

        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn loads_appended_bundle_from_single_file_orb() {
        let dir = tempfile::tempdir().unwrap();
        let orb_path = dir.path().join("test.orb");
        let bundle = build_test_bundle();
        let bytes = append_bundle_footer(b"fake-runtime".to_vec(), &bundle);
        std::fs::write(&orb_path, bytes).unwrap();

        let footer = read_appended_bundle_footer(&orb_path).unwrap().unwrap();
        assert_eq!(footer.offset, b"fake-runtime".len() as u64);
        assert_eq!(footer.length, bundle.len() as u64);

        let loaded = load_appended_orb_data(&orb_path).unwrap();
        assert!(loaded.security.password.is_none());
        let LoadedAssets::Plain(k) = loaded.assets else { panic!("expected plaintext") };
        assert_eq!(k.manifest.name, "test-orb");
        assert_eq!(k.documents.len(), 1);
        assert_eq!(k.chunks.len(), 1);
        assert_eq!(k.search.bm25.doc_count, 0);
        assert!(k.search.tfidf.is_none());
        assert!(k.search.trigram.is_none());
    }

    #[test]
    fn loads_sidecar_bundle_next_to_orb_binary() {
        let dir = tempfile::tempdir().unwrap();
        let orb_path = dir.path().join("test.orb");
        std::fs::write(&orb_path, b"fake-runtime").unwrap();

        let sidecar = sidecar_bundle_path(&orb_path);
        std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        std::fs::write(&sidecar, build_test_bundle()).unwrap();

        let loaded = load_sidecar_orb_data(&orb_path).unwrap();
        let LoadedAssets::Plain(k) = loaded.assets else { panic!("expected plaintext") };
        assert_eq!(k.manifest.name, "test-orb");
        assert_eq!(k.documents.len(), 1);
        assert_eq!(k.chunks.len(), 1);
        assert_eq!(k.search.bm25.doc_count, 0);
        assert!(k.search.tfidf.is_none());
        assert!(k.search.trigram.is_none());
    }

    /// Contract (plan §4.2): an assets dir carrying an optional `orb_security.json`
    /// loads both the knowledge and the password policy, and the policy actually
    /// gates — a wrong password is rejected, the right one unlocks.
    #[test]
    fn loads_assets_dir_with_security_json() {
        use std::io::Write as _;

        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();

        // Materialize a minimal plaintext assets dir.
        let manifest = OrbManifest {
            name: "guarded-orb".to_string(),
            version: "0.1.0".to_string(),
            description: "secured".to_string(),
            orb_format_version: "0.2".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            build_time: "2026-06-04T00:00:00Z".to_string(),
            source_documents: vec!["doc.pdf".to_string()],
            chunk_count: 1,
            index_format_version: "0.2".to_string(),
            binary_size_target_mb: 20,
            selected_retrieval_plan:
                mcporb_runtime_core::format::RetrievalPlanKind::Bm25Only,
            enabled_capabilities: vec![mcporb_runtime_core::format::Capability::Bm25],
            embedding_dim: None,
            embedding_model: None,
            embedding_model_tar_sha256: None,
            trigram_min_df: None,
            planning_rationale: vec![],
        };
        let documents = vec![Document {
            id: 0,
            title: "Doc".to_string(),
            source_path: "doc.pdf".to_string(),
            page_count: Some(1),
            sections: vec![],
        }];
        let chunks = vec![Chunk {
            id: 0,
            document_id: 0,
            section_id: None,
            page: Some(1),
            text: "guarded content".to_string(),
            token_count: 2,
        }];
        let write = |name: &str, bytes: &[u8]| {
            let mut f = std::fs::File::create(p.join(name)).unwrap();
            f.write_all(bytes).unwrap();
        };
        write("orb_manifest.json", &serde_json::to_vec(&manifest).unwrap());
        write("documents.postcard", &postcard::to_allocvec(&documents).unwrap());
        write("chunks.postcard", &postcard::to_allocvec(&chunks).unwrap());
        write(
            "bm25_index.postcard",
            &postcard::to_allocvec(&Bm25Index::default()).unwrap(),
        );
        write(
            "orb_security.json",
            security::test_security_json("dir-pw-strong").as_bytes(),
        );

        let loaded = load_orb_data(p).unwrap();
        let LoadedAssets::Plain(knowledge) = loaded.assets else { panic!("expected plaintext") };
        assert_eq!(knowledge.manifest.name, "guarded-orb");

        // The policy gates: starts locked, wrong password fails, right unlocks.
        let state = OrbState::new(
            loaded.security,
            LoadedAssets::Plain(knowledge),
            #[cfg(feature = "vector-embedder")]
            std::sync::Arc::new(mcporb_embed::ModelManager::with_cache_dir(
                tempfile::tempdir().unwrap().path().to_path_buf(),
            )),
            #[cfg(feature = "vector-embedder")]
            std::sync::Arc::new(mcporb_embed::empty_slot()),
            "GuiOnly".to_string(),
            None,
            None,
        );
        assert!(state.security.password_required());
        assert!(!state.security.is_unlocked());
        assert!(perform_unlock(&state, "wrong").is_err());
        assert!(perform_unlock(&state, "dir-pw-strong").is_ok());
        assert!(state.security.is_unlocked());
    }
}
