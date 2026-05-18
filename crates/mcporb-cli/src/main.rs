use clap::{Parser, Subcommand};
use std::path::PathBuf;
use mcporb_core::builder::{build_orb, OrbBuildConfig};
use mcporb_core::chunker::ChunkerConfig;
use mcporb_core::{OrbManifest, Chunk};
use mcporb_core::format::Bm25Index;
use mcporb_core::bm25_search;

#[derive(Parser)]
#[command(name = "mcporb", about = "MCPOrb — build and manage Orb binaries", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build an Orb from a source document
    Build {
        /// Path to the source document (Markdown or PDF)
        source: PathBuf,
        /// Output name for the Orb
        #[arg(short, long)]
        name: Option<String>,
        /// Output directory (default: ./target/orbs/<name>/)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Description for the Orb
        #[arg(short, long, default_value = "")]
        description: String,
    },
    /// Inspect a built Orb's manifest and stats
    Inspect {
        /// Path to the Orb output directory
        orb_dir: PathBuf,
    },
    /// List Orbs in the default output directory
    List,
    /// Run a built Orb (launches the runtime)
    Run {
        /// Path to the Orb output directory
        orb_dir: PathBuf,
        /// HTTP port (default: auto-select)
        #[arg(long)]
        port: Option<u16>,
        /// Auto-open browser
        #[arg(long)]
        open: bool,
        /// MCP stdio only mode
        #[arg(long)]
        stdio_only: bool,
        /// GUI only mode
        #[arg(long)]
        gui_only: bool,
    },
    /// Run a test query against a built Orb's BM25 index
    TestQuery {
        /// Path to the Orb output directory
        orb_dir: PathBuf,
        /// Search query
        query: String,
        /// Number of results to return
        #[arg(long, default_value = "5")]
        top_k: usize,
    },
    /// Package a built Orb directory into a single self-contained executable
    Package {
        /// Path to the Orb output directory
        orb_dir: PathBuf,
        /// Output executable path (default: <orb-dir>.orb)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Build a debug executable instead of a release executable
        #[arg(long)]
        debug: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { source, name, output, description } => {
            let name = name.unwrap_or_else(|| {
                source.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("orb")
                    .to_string()
            });
            let output_dir = output.unwrap_or_else(|| {
                PathBuf::from("target/orbs").join(&name)
            });

            println!("Building Orb '{}' from: {}", name, source.display());

            let result = build_orb(&source, OrbBuildConfig {
                name: name.clone(),
                description,
                output_dir: output_dir.clone(),
                chunker: ChunkerConfig::default(),
            })?;

            println!("✅ Build complete!");
            println!("   Name:        {}", result.manifest.name);
            println!("   Documents:   {}", result.documents.len());
            println!("   Chunks:      {}", result.chunks.len());
            println!("   Output:      {}", result.output_dir.display());

            println!("\nFirst chunks preview:");
            for chunk in result.chunks.iter().take(3) {
                let preview = &chunk.text[..chunk.text.len().min(120)];
                println!("  [{}] (page {:?}) {}…", chunk.id, chunk.page, preview);
            }

            Ok(())
        }

        Commands::Inspect { orb_dir } => {
            let manifest_path = orb_dir.join("orb_manifest.json");
            if !manifest_path.exists() {
                anyhow::bail!("No orb_manifest.json found in: {}", orb_dir.display());
            }
            let manifest_json = std::fs::read_to_string(&manifest_path)?;
            let manifest: OrbManifest = serde_json::from_str(&manifest_json)?;

            println!("Orb: {}", manifest.name);
            println!("  Version:       {}", manifest.version);
            println!("  Description:   {}", manifest.description);
            println!("  Build time:    {}", manifest.build_time);
            println!("  Chunks:        {}", manifest.chunk_count);
            println!("  Format ver:    {}", manifest.orb_format_version);
            println!("  Sources:       {}", manifest.source_documents.join(", "));

            let chunks_path = orb_dir.join("chunks.postcard");
            if chunks_path.exists() {
                let bytes = std::fs::read(&chunks_path)?;
                let chunks: Vec<Chunk> = postcard::from_bytes(&bytes)?;
                println!("  Chunks (verified): {}", chunks.len());
                println!("\nFirst 5 chunks:");
                for chunk in chunks.iter().take(5) {
                    let preview = &chunk.text[..chunk.text.len().min(100)];
                    println!("  [{}] page={:?} tokens={} | {}…",
                        chunk.id, chunk.page, chunk.token_count, preview);
                }
            }

            let index_path = orb_dir.join("bm25_index.postcard");
            if index_path.exists() {
                let bytes = std::fs::read(&index_path)?;
                let index: Bm25Index = postcard::from_bytes(&bytes)?;
                println!("\nBM25 Index:");
                println!("  Vocab size:    {}", index.vocab.len());
                println!("  Doc count:     {}", index.doc_count);
                println!("  Avg doc len:   {:.1} tokens", index.avg_doc_len);
            }

            Ok(())
        }

        Commands::List => {
            let orbs_dir = PathBuf::from("target/orbs");
            if !orbs_dir.exists() {
                println!("No Orbs found (target/orbs/ does not exist).");
                return Ok(());
            }
            let mut found = false;
            for entry in std::fs::read_dir(&orbs_dir)? {
                let entry = entry?;
                let manifest_path = entry.path().join("orb_manifest.json");
                if manifest_path.exists() {
                    let json = std::fs::read_to_string(&manifest_path)?;
                    let manifest: OrbManifest = serde_json::from_str(&json)?;
                    println!("  {} — {} chunks — built {}",
                        manifest.name, manifest.chunk_count, manifest.build_time);
                    found = true;
                }
            }
            if !found {
                println!("No Orbs found.");
            }
            Ok(())
        }

        Commands::Run { orb_dir, port, open, stdio_only, gui_only } => {
            if !orb_dir.join("orb_manifest.json").exists() {
                anyhow::bail!("No orb_manifest.json found in: {}", orb_dir.display());
            }

            // Find the mcporb-runtime binary
            // Try sibling of current executable first, then fall back to cargo run
            let runtime_bin = find_runtime_binary()?;

            let mut cmd = std::process::Command::new(&runtime_bin);
            cmd.arg("--assets").arg(&orb_dir);

            if stdio_only {
                cmd.arg("--stdio-only");
            } else if gui_only {
                cmd.arg("--gui-only");
            }
            if open {
                cmd.arg("--open");
            }
            if let Some(p) = port {
                cmd.arg("--port").arg(p.to_string());
            }

            println!("Launching Orb from: {}", orb_dir.display());
            println!("Runtime: {}", runtime_bin.display());

            let status = cmd.status()?;
            if !status.success() {
                anyhow::bail!("Runtime exited with status: {}", status);
            }
            Ok(())
        }

        Commands::TestQuery { orb_dir, query, top_k } => {
            if !orb_dir.join("orb_manifest.json").exists() {
                anyhow::bail!("No orb_manifest.json found in: {}", orb_dir.display());
            }

            // Load chunks
            let chunks_bytes = std::fs::read(orb_dir.join("chunks.postcard"))?;
            let chunks: Vec<Chunk> = postcard::from_bytes(&chunks_bytes)?;

            // Load BM25 index
            let index_bytes = std::fs::read(orb_dir.join("bm25_index.postcard"))?;
            let index: Bm25Index = postcard::from_bytes(&index_bytes)?;

            println!("Query: \"{}\"", query);
            println!("Top-{top_k} results from {} chunks:\n", chunks.len());

            let results = bm25_search(&index, &query, top_k);

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            for (rank, (chunk_id, score)) in results.iter().enumerate() {
                if let Some(chunk) = chunks.get(*chunk_id as usize) {
                    let preview = &chunk.text[..chunk.text.len().min(300)];
                    println!("#{} [Score: {score:.3}] Chunk {} | Page {:?}",
                        rank + 1, chunk.id, chunk.page);
                    println!("   {}", preview.replace('\n', " "));
                    println!();
                }
            }

            Ok(())
        }

        Commands::Package { orb_dir, output, debug } => {
            let manifest = load_orb_manifest(&orb_dir)?;
            let orb_dir = std::fs::canonicalize(&orb_dir)?;
            let output_path = output.unwrap_or_else(|| orb_dir.with_extension("orb"));
            let build_profile = if debug { "debug" } else { "release" };
            let workspace_root = workspace_root()?;

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            println!("Packaging Orb '{}'", manifest.name);
            println!("  Assets:   {}", orb_dir.display());
            println!("  Output:   {}", output_path.display());
            println!("  Profile:  {build_profile}");

            let mut cmd = std::process::Command::new("cargo");
            cmd.arg("build").arg("-p").arg("mcporb-runtime");
            if !debug {
                cmd.arg("--release");
            }
            cmd.current_dir(&workspace_root)
                .env("MCPORB_EMBED_ASSETS_DIR", &orb_dir);

            let status = cmd.status()?;
            if !status.success() {
                anyhow::bail!("Packaging build failed with status: {}", status);
            }

            let runtime_path = workspace_root.join("target").join(build_profile).join("mcporb-runtime");
            if !runtime_path.exists() {
                anyhow::bail!("Built runtime not found at: {}", runtime_path.display());
            }

            std::fs::copy(&runtime_path, &output_path)?;
            set_executable_bit(&output_path)?;

            println!("✅ Package complete!");
            println!("   Executable:   {}", output_path.display());
            println!("   Embedded Orb: {}", manifest.name);
            println!("   Run:          {} --gui-only --open", output_path.display());

            Ok(())
        }
    }
}

fn load_orb_manifest(orb_dir: &std::path::Path) -> anyhow::Result<OrbManifest> {
    let manifest_path = orb_dir.join("orb_manifest.json");
    if !manifest_path.exists() {
        anyhow::bail!("No orb_manifest.json found in: {}", orb_dir.display());
    }

    let manifest_json = std::fs::read_to_string(manifest_path)?;
    Ok(serde_json::from_str(&manifest_json)?)
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|path| path.parent())
        .map(|path| path.to_path_buf())
        .ok_or_else(|| anyhow::anyhow!("failed to determine workspace root from {}", crate_dir.display()))
}

fn set_executable_bit(path: &std::path::Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }

    Ok(())
}

fn find_runtime_binary() -> anyhow::Result<PathBuf> {
    // Strategy 1: sibling of current executable (installed scenario)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("mcporb-runtime");
            if sibling.exists() {
                return Ok(sibling);
            }
            // Windows
            let sibling_exe = dir.join("mcporb-runtime.exe");
            if sibling_exe.exists() {
                return Ok(sibling_exe);
            }
        }
    }

    // Strategy 2: development build artifacts
    let dev_bin = PathBuf::from("target/debug/mcporb-runtime");
    if dev_bin.exists() {
        return Ok(dev_bin);
    }
    let release_bin = PathBuf::from("target/release/mcporb-runtime");
    if release_bin.exists() {
        return Ok(release_bin);
    }

    anyhow::bail!(
        "mcporb-runtime binary not found. Build it first with: cargo build -p mcporb-runtime"
    )
}
