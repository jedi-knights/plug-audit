//! CLI entrypoint. Full command surface (`check`, `--fix`, `--config`, etc.)
//! lands in PA-6; this stub exists so the binary compiles from scaffold onward.

use std::process::ExitCode;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "plug-audit", version, about = "Static analyzer for Neovim plugin repos", long_about = None)]
struct Cli {
    /// Placeholder — real subcommands land in PA-6.
    #[arg(long, hide = true)]
    _stub: bool,
}

pub fn run() -> ExitCode {
    let _cli = Cli::parse();
    eprintln!("plug-audit: scaffold-only build. Rule engine lands in PA-2 through PA-6.");
    ExitCode::from(0)
}
