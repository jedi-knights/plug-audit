//! Wrap `tree-sitter-lua` behind a small, allocation-conservative API.
//!
//! Design choices:
//!
//! - The parser is `!Sync` (tree-sitter parsers hold thread-local state)
//!   so callers construct one per thread. Rule engines that fan out across
//!   files can pool parsers per worker.
//! - Traversal uses tree-sitter's built-in `TreeCursor` — no recursion,
//!   so the depth bound is O(source_len) implicitly. Per the
//!   algorithmic-complexity rule, AST walks over user-controlled input
//!   must not recurse without a named bound.
//! - Parse errors are surfaced as `ERROR` / MISSING nodes in the tree
//!   (`tree-sitter`'s error-recovery model), not as a hard failure of
//!   [`LuaParser::parse`]. Callers check [`ParsedTree::has_syntax_errors`]
//!   or iterate [`ParsedTree::error_spans`] to decide how to route them.
//!   Routing to a `plug-audit/parse-error` diagnostic vs. a category we
//!   would need to add to the ontology is a PA-6 concern — the parser
//!   only exposes the raw information.

use crate::domain::finding::ByteSpan;
use tree_sitter::{Node, Parser, Tree, TreeCursor};

/// A configured `tree-sitter-lua` parser.
pub struct LuaParser {
    parser: Parser,
}

impl LuaParser {
    /// Build a parser with the Lua grammar loaded.
    pub fn new() -> Result<Self, LuaParserError> {
        let mut parser = Parser::new();
        let language: tree_sitter::Language = tree_sitter_lua::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(LuaParserError::LanguageInit)?;
        Ok(Self { parser })
    }

    /// Parse `source` into a [`ParsedTree`]. Returns `Err` only when
    /// the parser cannot produce a tree at all (cancellation / OOM);
    /// syntactically-invalid input parses successfully with `ERROR`
    /// nodes and is inspected via [`ParsedTree::has_syntax_errors`].
    pub fn parse(&mut self, source: &str) -> Result<ParsedTree, LuaParserError> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or(LuaParserError::ParseAborted)?;
        Ok(ParsedTree {
            tree,
            source_len: source.len(),
        })
    }
}

impl std::fmt::Debug for LuaParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaParser").finish_non_exhaustive()
    }
}

/// A parsed tree plus the source-length metadata callers need to
/// validate spans without re-hashing the source.
#[derive(Debug)]
pub struct ParsedTree {
    tree: Tree,
    source_len: usize,
}

impl ParsedTree {
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    pub fn source_len(&self) -> usize {
        self.source_len
    }

    /// True if any node in the tree is an ERROR or MISSING node.
    pub fn has_syntax_errors(&self) -> bool {
        self.tree.root_node().has_error()
    }

    /// Byte spans of every ERROR / MISSING node, in pre-order. Bounded
    /// by tree size, itself bounded by `source_len`.
    pub fn error_spans(&self) -> Vec<ByteSpan> {
        all_nodes(&self.tree)
            .into_iter()
            .filter(|n| n.is_error() || n.is_missing())
            .map(|n| ByteSpan::new(n.start_byte(), n.end_byte()))
            .collect()
    }
}

/// Collect all nodes in the tree in pre-order (parent before children,
/// left-to-right siblings). Bounded by tree size.
///
/// This is the primary walk API rules consume; the recursion-free
/// implementation uses tree-sitter's [`TreeCursor`] to satisfy the
/// bounded-loop discipline for AST walks.
pub fn all_nodes<'tree>(tree: &'tree Tree) -> Vec<Node<'tree>> {
    let mut cursor = tree.walk();
    collect_from_cursor(&mut cursor)
}

fn collect_from_cursor<'tree>(cursor: &mut TreeCursor<'tree>) -> Vec<Node<'tree>> {
    let mut out = Vec::new();
    loop {
        out.push(cursor.node());
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return out;
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LuaParserError {
    #[error("failed to initialize tree-sitter-lua grammar: {0}")]
    LanguageInit(#[source] tree_sitter::LanguageError),

    #[error("tree-sitter parse aborted (cancellation or resource limit)")]
    ParseAborted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn read_fixture(rel: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample-repo")
            .join(rel);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
    }

    #[test]
    fn parses_empty_source() {
        let mut parser = LuaParser::new().unwrap();
        let tree = parser.parse("").unwrap();
        assert!(!tree.has_syntax_errors());
        assert_eq!(tree.error_spans(), vec![]);
        assert_eq!(tree.source_len(), 0);
    }

    #[test]
    fn parses_trivial_source() {
        let mut parser = LuaParser::new().unwrap();
        let src = "local x = 1\nreturn x\n";
        let tree = parser.parse(src).unwrap();
        assert!(!tree.has_syntax_errors());
        assert_eq!(tree.root_node().kind(), "chunk");
    }

    #[test]
    fn detects_syntax_error() {
        let mut parser = LuaParser::new().unwrap();
        // Missing `end` keyword — tree-sitter recovers but flags the tree.
        let tree = parser.parse("function foo()\n  return 1\n").unwrap();
        assert!(tree.has_syntax_errors());
        assert!(!tree.error_spans().is_empty());
    }

    #[test]
    fn all_nodes_visits_root_first() {
        let mut parser = LuaParser::new().unwrap();
        let tree = parser.parse("return 1").unwrap();
        let nodes = all_nodes(tree.tree());
        assert_eq!(nodes[0].kind(), "chunk");
        assert!(nodes.len() >= 2, "chunk plus at least a return statement");
    }

    #[test]
    fn all_nodes_bounded_by_source_size() {
        let mut parser = LuaParser::new().unwrap();
        let src = "return 1";
        let tree = parser.parse(src).unwrap();
        let nodes = all_nodes(tree.tree());
        assert!(
            nodes.len() < 256,
            "8-byte source produced {} nodes — tree walker likely unbounded",
            nodes.len()
        );
    }

    #[test]
    fn parses_go_task_plugin_fixture() {
        let mut parser = LuaParser::new().unwrap();
        let src = read_fixture("plugin/go-task.lua");
        let tree = parser.parse(&src).unwrap();
        assert!(
            !tree.has_syntax_errors(),
            "real jedi-knights source failed to parse cleanly: {:?}",
            tree.error_spans()
        );
    }

    #[test]
    fn parses_go_init_fixture() {
        let mut parser = LuaParser::new().unwrap();
        let src = read_fixture("lua/go/init.lua");
        let tree = parser.parse(&src).unwrap();
        assert!(!tree.has_syntax_errors());
    }

    #[test]
    fn parses_go_config_fixture() {
        let mut parser = LuaParser::new().unwrap();
        let src = read_fixture("lua/go/config.lua");
        let tree = parser.parse(&src).unwrap();
        assert!(!tree.has_syntax_errors());
    }

    #[test]
    fn parses_pytest_parser_fixture() {
        let mut parser = LuaParser::new().unwrap();
        let src = read_fixture("lua/pytest/parser.lua");
        let tree = parser.parse(&src).unwrap();
        assert!(!tree.has_syntax_errors());
    }

    #[test]
    fn parses_tests_helpers_fixture() {
        let mut parser = LuaParser::new().unwrap();
        let src = read_fixture("tests/helpers.lua");
        let tree = parser.parse(&src).unwrap();
        assert!(!tree.has_syntax_errors());
    }

    #[test]
    fn debug_impl_hides_parser_internals() {
        let parser = LuaParser::new().unwrap();
        let debug = format!("{parser:?}");
        assert!(debug.contains("LuaParser"));
    }
}
