// Binary size spike — measures baseline stripped release size with all core deps linked.
// Build with: cargo build --release -p mcporb-size-spike
// Then measure: ls -lh target/release/mcporb-size-spike
// Target: ≤ 15MB stripped. Record result in dev-plan.md §3.1.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("size spike binary — all deps linked");
    Ok(())
}
