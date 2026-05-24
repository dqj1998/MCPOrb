//! Smoke test: load a local model dir, embed a query, report stats + latency.
//!
//! Usage:
//!   cargo run --release -p mcporb-embed --example smoke -- <model_dir> [text]
//!
//! Expects model_dir to contain:
//!   - model_f16.onnx
//!   - tokenizer.json

use std::path::Path;
use std::time::Instant;

use mcporb_embed::TractEmbedder;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let model_dir = args
        .get(1)
        .expect("usage: smoke <model_dir> [text]");
    let text = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "What is the capital of France?".to_string());

    eprintln!("Loading embedder from {:?}...", model_dir);
    let t0 = Instant::now();
    let embedder = TractEmbedder::load(Path::new(model_dir))?;
    eprintln!("  loaded + optimized in {:?}", t0.elapsed());

    eprintln!("Warmup (3 runs)...");
    for _ in 0..3 {
        let _ = embedder.embed_sync(&text)?;
    }

    let n = 20usize;
    eprintln!("Measuring ({} runs)...", n);
    let mut times = Vec::with_capacity(n);
    let mut last = vec![];
    for _ in 0..n {
        let t = Instant::now();
        last = embedder.embed_sync(&text)?;
        times.push(t.elapsed());
    }

    times.sort();
    let p50 = times[n / 2];
    let p95_idx = ((n as f64) * 0.95) as usize;
    let p99_idx = (((n as f64) * 0.99) as usize).min(n - 1);
    let p95 = times[p95_idx.min(n - 1)];
    let p99 = times[p99_idx];

    let norm: f32 = last.iter().map(|x| x * x).sum::<f32>().sqrt();

    println!("=== smoke test ===");
    println!("query:        {:?}", text);
    println!("output dim:   {}  (expect {})", last.len(), mcporb_embed::MODEL_DIM);
    println!("output norm:  {:.6}  (expect ≈ 1.0)", norm);
    println!("first 8:      {:?}", &last[..8]);
    println!("latency p50:  {:?}", p50);
    println!("latency p95:  {:?}", p95);
    println!("latency p99:  {:?}", p99);

    Ok(())
}
