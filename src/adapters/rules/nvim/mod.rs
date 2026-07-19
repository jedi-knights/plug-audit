//! Rules in the `nvim/` category — Neovim plugin runtime patterns.

pub mod augroup_clear;
pub mod health_check;
pub mod plug_mapping;

pub use augroup_clear::AugroupClear;
pub use health_check::HealthCheck;
pub use plug_mapping::PlugMapping;
