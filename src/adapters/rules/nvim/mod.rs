//! Rules in the `nvim/` category — Neovim plugin runtime patterns.

pub mod augroup_clear;
pub mod health_check;

pub use augroup_clear::AugroupClear;
pub use health_check::HealthCheck;
