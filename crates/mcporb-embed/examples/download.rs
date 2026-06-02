//! End-to-end test of ModelManager::download() against the real MODEL_URLS.
//!
//! Usage:
//!   cargo run --release -p mcporb-embed --example download

use mcporb_embed::ModelManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cache = std::env::temp_dir().join(format!("mcporb-dl-test-{}", std::process::id()));
    let mgr = ModelManager::with_cache_dir(cache.clone());
    println!("cache: {:?}", mgr.cache_dir());
    println!("is_ready (before): {}", mgr.is_ready());

    let used_url = mgr.download().await?;
    println!("downloaded from: {}", used_url);
    println!("is_ready (after):  {}", mgr.is_ready());

    let current = mgr.current_dir();
    for name in [
        "model_f16.onnx",
        "tokenizer.json",
        "special_tokens_map.json",
        "config.json",
        ".source_sha256",
    ] {
        let p = current.join(name);
        let sz = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        println!("  {:<30} {} bytes", name, sz);
    }

    println!("\ncleaning up {:?}", cache);
    std::fs::remove_dir_all(&cache)?;
    Ok(())
}
