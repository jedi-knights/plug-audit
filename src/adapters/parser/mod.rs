//! Lua parser adapter — wraps `tree-sitter-lua`.

pub mod lua;

pub use lua::{LuaParser, LuaParserError, ParsedTree, all_nodes};
