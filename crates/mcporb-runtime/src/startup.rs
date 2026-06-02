use clap::Parser;
use std::io::IsTerminal;

#[derive(Debug, Clone, PartialEq)]
pub enum StartupMode {
    AllGui,
    GuiOnly,
    StdioOnly,
}

#[derive(Parser, Debug)]
#[command(name = "orb", about = "MCPOrb — self-contained knowledge Orb")]
pub struct OrbArgs {
    #[arg(long)]
    pub all_gui: bool,
    #[arg(long, hide = true)]
    pub stdio_gui: bool,
    #[arg(long)]
    pub gui_only: bool,
    #[arg(long)]
    pub stdio_only: bool,
    #[arg(long)]
    pub open: bool,
    #[arg(long)]
    pub no_open: bool,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub assets: Option<std::path::PathBuf>,
}

pub struct StartupConfig {
    pub mode: StartupMode,
    pub auto_open: bool,
    pub port: Option<u16>,
    pub assets_path: Option<std::path::PathBuf>,
}

pub fn detect_startup(args: &OrbArgs) -> StartupConfig {
    let mode = if args.stdio_only {
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
        assets_path: args.assets.clone(),
    }
}
