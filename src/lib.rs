//! plug-audit — static analyzer for Neovim plugin repos.
//!
//! Hexagonal architecture (ports and adapters):
//!
//! - [`domain`] — pure types (rules, findings, severities, locations).
//! - [`ports`] — traits the domain depends on (parser, repo discovery, reporter).
//! - [`adapters`] — concrete impls (tree-sitter Lua parser, walkdir discovery, console/JSON reporter).
//! - [`cli`] — clap-based entrypoint; wires adapters into the domain.

pub mod adapters;
pub mod cli;
pub mod domain;
pub mod ports;
