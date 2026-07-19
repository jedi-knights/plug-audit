//! CLI entrypoint. Owns the top-level `Cli` struct and the exit-code
//! contract; delegates the actual work to per-subcommand modules.
//!
//! Exit codes (locked contract):
//! - `0` — success, findings-only. The tool ran end-to-end and reported
//!   what it found; the caller decides what to do next.
//! - `1` — tool error. The tool could not complete its work (discovery
//!   or parser initialization failed). Findings, if any, are still
//!   emitted before we exit.
//! - `2` — reserved for `--strict` mode plus at least one Must Fix
//!   finding. Distinct from `1` so CI can tell "tool broke" apart from
//!   "your code broke."

use std::process::ExitCode;

use clap::{Parser, Subcommand};

pub mod check;
pub mod report;

#[derive(Parser, Debug)]
#[command(
    name = "plug-audit",
    version,
    about = "Static analyzer for Neovim plugin repos"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Scan a Neovim plugin repo and report rule violations.
    Check(check::CheckArgs),
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check(args) => check::run(&args),
    }
}
