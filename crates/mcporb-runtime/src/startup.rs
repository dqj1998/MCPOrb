use clap::Parser;
use std::io::IsTerminal;

#[derive(Debug, Clone, PartialEq)]
pub enum StartupMode {
    AllGui,
    GuiOnly,
    StdioOnly,
}

#[derive(Parser, Debug)]
#[command(name = "orb", about = "MCPOrb Runner — self-contained knowledge Orb")]
pub struct OrbArgs {
    #[arg(long)]
    pub all_gui: bool,
    #[arg(long, hide = true)]
    pub stdio_gui: bool,
    #[arg(long)]
    pub gui_only: bool,
    #[arg(long)]
    pub stdio_only: bool,
    /// Alias for --stdio-only, accepted for consistency with the
    /// mcporb-runner wrapper mode (plan GUI-STDIO-Runner.md).
    #[arg(long, hide = true)]
    pub mcp_stdio: bool,
    #[arg(long)]
    pub open: bool,
    #[arg(long)]
    pub no_open: bool,
    #[arg(long)]
    pub port: Option<u16>,
    /// Fixed token for the HTTP MCP endpoint. When set, the token is used
    /// instead of generating a random one (used by the Runtime App).
    #[arg(long)]
    pub token: Option<String>,
    #[arg(long)]
    pub assets: Option<std::path::PathBuf>,
    /// Load an Orb ZIP bundle directly. Intended for the installed Runtime App
    /// and MCP client STDIO shims; leaves legacy self-contained .orb behavior intact.
    #[arg(long)]
    pub orb_zip: Option<std::path::PathBuf>,
    /// Remember this Orb's password on this device (OS keychain), then exit.
    /// Prompts once; future launches unlock without prompting. Only meaningful
    /// for Orbs packaged with --remember-unlock.
    #[arg(long)]
    pub unlock: bool,
    /// Orb identifier for metrics persistence. When set, metrics are written
    /// to {metrics_dir}/{orb_id}.json on each request/search.
    #[arg(long)]
    pub orb_id: Option<String>,
    /// Directory for persisted metrics files. Requires --orb-id.
    #[arg(long)]
    pub metrics_dir: Option<std::path::PathBuf>,
    /// Bind to 0.0.0.0 instead of 127.0.0.1, allowing external network access
    /// to the HTTP server. Use with caution — the local Web UI was designed
    /// for loopback-only access.
    #[arg(long)]
    pub bind_external: bool,
    /// Override the transport label recorded for MCP requests (e.g. "http"
    /// when running under the gateway). When unset, defaults to "stdio" for
    /// stdio-only mode.
    #[arg(long)]
    pub mcp_transport: Option<String>,
}

pub struct StartupConfig {
    pub mode: StartupMode,
    pub auto_open: bool,
    pub port: Option<u16>,
    pub token: Option<String>,
    pub assets_path: Option<std::path::PathBuf>,
    pub orb_zip_path: Option<std::path::PathBuf>,
    pub orb_id: Option<String>,
    pub metrics_dir: Option<std::path::PathBuf>,
    pub bind_external: bool,
    pub mcp_transport: Option<String>,
}

pub fn detect_startup(args: &OrbArgs) -> StartupConfig {
    let mode = if args.stdio_only || args.mcp_stdio {
        StartupMode::StdioOnly
    } else if args.gui_only {
        StartupMode::GuiOnly
    } else if args.all_gui || args.stdio_gui {
        StartupMode::AllGui
    } else {
        if std::io::stdin().is_terminal() {
            StartupMode::GuiOnly
        } else {
            StartupMode::AllGui
        }
    };

    let auto_open = if args.no_open {
        false
    } else if args.open {
        true
    } else {
        mode == StartupMode::GuiOnly
    };

    StartupConfig {
        mode,
        auto_open,
        port: args.port,
        token: args.token.clone(),
        assets_path: args.assets.clone(),
        orb_zip_path: args.orb_zip.clone(),
        orb_id: args.orb_id.clone(),
        metrics_dir: args.metrics_dir.clone(),
        bind_external: args.bind_external,
        mcp_transport: args.mcp_transport.clone(),
    }
}
