//! Repo discovery adapter.

pub mod discovery;

pub use discovery::{DiscoveryError, LuaFile, LuaFileRole, classify, discover};
