//! Adapters — concrete implementations of the tool's I/O surfaces.
//!
//! - [`repo`] walks a directory and classifies each `.lua` file by role.
//! - [`parser`] wraps `tree-sitter-lua` and exposes a bounded tree walker.
//!
//! Ports (trait boundaries between the domain and these adapters) are
//! deferred until PA-4 — there is only one impl of each surface today,
//! and per the design-patterns rule we do not introduce abstraction
//! without a signal.

pub mod parser;
pub mod repo;
