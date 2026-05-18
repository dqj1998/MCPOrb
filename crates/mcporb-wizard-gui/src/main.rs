//! MCPOrb Wizard GUI — Tauri-based Orb builder wizard.
//!
//! Phase 5 scaffold: this binary will become a Tauri desktop application
//! in a future release. For now it prints usage information.
//!
//! Planned features (v0.4+):
//! - File picker to select source documents (PDF/Markdown)
//! - Progress display during `mcporb build`
//! - Preview of generated chunks and manifest
//! - One-click Orb generation without CLI knowledge

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("MCPOrb Wizard GUI — scaffold (Tauri integration pending)");

    println!("MCPOrb Wizard GUI");
    println!("=================");
    println!();
    println!("This is a placeholder for the upcoming Tauri-based Orb builder wizard.");
    println!("For now, use the CLI:");
    println!();
    println!("  mcporb build <source>          Build an Orb from a PDF or Markdown file");
    println!("  mcporb inspect <orb-dir>       Inspect a built Orb");
    println!("  mcporb run <orb-dir> --open    Launch an Orb and open the Web UI");
    println!("  mcporb test-query <orb-dir> <query>  Test BM25 search");
    println!("  mcporb list                    List all built Orbs");
    println!();
    println!("Tauri GUI integration is planned for v0.4.");

    Ok(())
}
