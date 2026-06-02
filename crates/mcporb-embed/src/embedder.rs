//! tract-based sentence embedder. See spec §5.5.
//!
//! Sequence length is fixed at [`MAX_SEQ_LEN`] at load time so tract can do
//! static shape optimization. Queries longer than this are truncated.
//!
//! Synchronous inference is ~25-40ms on a modern CPU; callers must use
//! [`embed`] (which wraps via `spawn_blocking`) from async contexts.

use anyhow::{anyhow, Context, Result};
use arc_swap::ArcSwap;
use std::path::Path;
use std::sync::Arc;
use tokenizers::Tokenizer;
use tract_onnx::prelude::tract_ndarray::Array2;
use tract_onnx::prelude::*;

use crate::model::{MAX_SEQ_LEN, MODEL_DIM};

type RunnablePlan = Arc<TypedRunnableModel>;

pub struct TractEmbedder {
    plan: RunnablePlan,
    tokenizer: Tokenizer,
}

impl TractEmbedder {
    /// Load an ONNX model + tokenizer from a directory laid out by the
    /// downloader (model_f16.onnx + tokenizer.json + ...).
    pub fn load(model_dir: &Path) -> Result<Self> {
        let model_path = model_dir.join("model_f16.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        let seq = MAX_SEQ_LEN as i64;

        // Model has 3 inputs (input_ids, attention_mask, token_type_ids) — all
        // shape [batch_size, sequence_length]. Pin all three to (1, MAX_SEQ_LEN)
        // so tract can do static shape optimization.
        let plan = tract_onnx::onnx()
            .model_for_path(&model_path)
            .with_context(|| format!("loading ONNX from {:?}", model_path))?
            .with_input_fact(0, InferenceFact::dt_shape(i64::datum_type(), tvec!(1, seq)))?
            .with_input_fact(1, InferenceFact::dt_shape(i64::datum_type(), tvec!(1, seq)))?
            .with_input_fact(2, InferenceFact::dt_shape(i64::datum_type(), tvec!(1, seq)))?
            .into_optimized()?
            .into_runnable()?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("loading tokenizer from {:?}: {}", tokenizer_path, e))?;

        Ok(Self { plan, tokenizer })
    }

    /// Synchronous CPU inference. Callers in async contexts must wrap via
    /// `spawn_blocking` (see [`embed`]).
    pub fn embed_sync(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow!("tokenize: {}", e))?;

        let mut ids = vec![0i64; MAX_SEQ_LEN];
        let mut mask = vec![0i64; MAX_SEQ_LEN];

        for (i, &id) in encoding.get_ids().iter().take(MAX_SEQ_LEN).enumerate() {
            ids[i] = id as i64;
            mask[i] = 1;
        }

        // token_type_ids = all zeros for sentence-transformer single-sentence use.
        let token_type_ids = vec![0i64; MAX_SEQ_LEN];

        let ids_t: Tensor = Array2::from_shape_vec((1, MAX_SEQ_LEN), ids)?.into_tensor();
        let mask_t: Tensor = Array2::from_shape_vec((1, MAX_SEQ_LEN), mask.clone())?.into_tensor();
        let tti_t: Tensor = Array2::from_shape_vec((1, MAX_SEQ_LEN), token_type_ids)?.into_tensor();

        let outputs = self
            .plan
            .run(tvec!(ids_t.into(), mask_t.into(), tti_t.into()))?;
        // Model output may be F16 (when model is F16-quantized). Cast to F32
        // for pooling math, which is cheap and keeps downstream code uniform.
        let hidden_tensor = outputs[0].cast_to_dt(f32::datum_type())?;
        let hidden = hidden_tensor.to_plain_array_view::<f32>()?;
        // hidden shape: [1, seq_len, hidden_size=MODEL_DIM]

        // Mean pooling weighted by attention_mask (pad tokens excluded).
        let mut pooled = vec![0.0f32; MODEL_DIM];
        let mut count: f32 = 0.0;
        for (t, &m) in mask.iter().enumerate() {
            if m == 0 {
                continue;
            }
            count += 1.0;
            for d in 0..MODEL_DIM {
                pooled[d] += hidden[[0, t, d]];
            }
        }
        if count == 0.0 {
            return Err(anyhow!("empty tokenization (all-pad mask)"));
        }
        for v in pooled.iter_mut() {
            *v /= count;
        }

        // L2 normalize.
        let norm = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in pooled.iter_mut() {
                *v /= norm;
            }
        }
        Ok(pooled)
    }
}

/// Async-friendly wrapper. Hands off the synchronous ~25-40ms inference to
/// the blocking thread pool so the tokio worker stays free.
pub async fn embed(embedder: Arc<TractEmbedder>, text: String) -> Result<Vec<f32>> {
    tokio::task::spawn_blocking(move || embedder.embed_sync(&text))
        .await
        .map_err(|e| anyhow!("blocking task join: {}", e))?
}

/// Hot-swappable slot used by the Runtime. Starts as `None`; the downloader
/// populates it once the model is on disk. Query path reads via `load_full()`
/// for a lock-free snapshot.
pub type EmbedderSlot = ArcSwap<Option<Arc<TractEmbedder>>>;

/// Construct an empty (None) `EmbedderSlot`. Provided here so the Runtime
/// crate doesn't need a direct dep on `arc-swap`.
pub fn empty_slot() -> EmbedderSlot {
    ArcSwap::new(Arc::new(None))
}
