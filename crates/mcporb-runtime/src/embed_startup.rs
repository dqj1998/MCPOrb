//! Wires the query-embedder lifecycle into Runtime startup.
//!
//! See spec §5.7. Behavior:
//!   - If the loaded Orb has no `embedding_model_tar_sha256` in its manifest,
//!     no embedder work is done (legacy or BM25-only Orb).
//!   - If the manifest SHA matches `mcporb_embed::MODEL_TAR_SHA256`:
//!     - cache-hit (`is_ready`) → synchronous load (~650ms)
//!     - cache-miss → spawn background download; hot-load on success
//!   - Mismatched SHA: leave slot empty; the request-path will surface a clear
//!     error (per §4.5 row 3 of the downgrade matrix).

use std::sync::Arc;

use mcporb_embed::{empty_slot, EmbedderSlot, ModelManager, TractEmbedder};
use mcporb_runtime_core::OrbManifest;

/// Build the manager + slot pair, kicking off background download if needed.
/// Always returns valid handles; the slot may stay empty if the Orb declares
/// no dense embedding or runs into a model mismatch.
pub fn prepare(manifest: &OrbManifest) -> anyhow::Result<(Arc<ModelManager>, Arc<EmbedderSlot>)> {
    let (mm, slot) = prepare_empty()?;
    start_for_manifest(mm.clone(), slot.clone(), manifest);
    Ok((mm, slot))
}

/// Build the manager + (empty) slot without consulting a manifest. Used at
/// startup for asset-encrypted Orbs, whose manifest is not available until the
/// payload is decrypted on unlock; [`start_for_manifest`] is then called from
/// the unlock path (plan §4.4).
pub fn prepare_empty() -> anyhow::Result<(Arc<ModelManager>, Arc<EmbedderSlot>)> {
    let mm = Arc::new(ModelManager::new()?);
    let slot: Arc<EmbedderSlot> = Arc::new(empty_slot());
    Ok((mm, slot))
}

/// Inspect the manifest and, if it declares the runtime's embedding model, load
/// it (cache-hit) or kick off a background download (cache-miss) into `slot`.
/// Safe to call once the manifest is known (startup for plaintext Orbs, unlock
/// for encrypted ones). Requires a Tokio runtime (uses `tokio::spawn`).
pub fn start_for_manifest(mm: Arc<ModelManager>, slot: Arc<EmbedderSlot>, manifest: &OrbManifest) {
    let Some(manifest_sha) = manifest.embedding_model_tar_sha256.as_deref() else {
        tracing::debug!("orb manifest has no embedding_model_tar_sha256; skipping embedder load");
        return;
    };

    if manifest_sha != mcporb_embed::MODEL_TAR_SHA256 {
        tracing::warn!(
            orb_sha = %manifest_sha,
            runtime_sha = %mcporb_embed::MODEL_TAR_SHA256,
            "manifest embedding SHA mismatches runtime — dense queries will be rejected"
        );
        return;
    }

    if mm.is_ready() {
        let dir = mm.current_dir();
        match TractEmbedder::load(&dir) {
            Ok(emb) => {
                slot.store(Arc::new(Some(Arc::new(emb))));
                tracing::info!("embedder loaded from cache; vector method ready");
            }
            Err(e) => {
                tracing::warn!(error = %e, "cached model present but load failed; will redownload");
                spawn_background_download(mm, slot);
            }
        }
    } else {
        tracing::info!("embedder model not cached; starting background download (~220MB)");
        spawn_background_download(mm, slot);
    }
}

fn spawn_background_download(mm: Arc<ModelManager>, slot: Arc<EmbedderSlot>) {
    tokio::spawn(async move {
        match mm.download().await {
            Ok(url) => {
                tracing::info!(%url, "embedder model downloaded");
                let dir = mm.current_dir();
                match TractEmbedder::load(&dir) {
                    Ok(emb) => {
                        slot.store(Arc::new(Some(Arc::new(emb))));
                        tracing::info!("embedder hot-loaded; vector method now ready");
                    }
                    Err(e) => tracing::error!(error = %e, "embedder load failed after download"),
                }
            }
            Err(e) => tracing::warn!(
                error = %e,
                "model download failed — vector method will downgrade to auto"
            ),
        }
    });
}
