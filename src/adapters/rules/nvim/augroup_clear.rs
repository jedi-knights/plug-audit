//! `nvim/augroup-clear` — augroups must pass `{ clear = true }`.
//!
//! Source pattern: universal jedi-knights and community convention. An
//! augroup created without an explicit `clear = true` accumulates
//! duplicate autocmds every time the plugin file is re-sourced (`:source`
//! at develop time, `:PackerCompile` at install time, etc.). The bug is
//! silent — nothing errors, autocmds just fire more times per event.
//!
//! Detection: any `function_call` whose function-name text is exactly
//! `vim.api.nvim_create_augroup` (the qualified form) that is either
//! - called with fewer than two positional arguments, or
//! - called with a second arg that is not a `table_constructor` holding
//!   the field `clear = true`.
//!
//! Auto-fix: not implemented in v0.1.0 (the second-arg table may need
//! to be constructed from scratch or merged). Fix guidance is manual.

use std::sync::LazyLock;

use tree_sitter::Node;

use crate::adapters::parser::all_nodes;
use crate::domain::finding::{ByteSpan, Finding};
use crate::domain::rule::{FixGuidance, RuleId};
use crate::domain::rule_engine::{LintContext, LintRule};
use crate::domain::severity::Severity;

static ID: LazyLock<RuleId> = LazyLock::new(|| RuleId::parse("nvim/augroup-clear").unwrap());

static FIX: LazyLock<FixGuidance> = LazyLock::new(|| FixGuidance::Manual {
    description:
        "add `{ clear = true }` as the second argument to nvim_create_augroup so re-sourcing \
        the file replaces the augroup instead of appending to it"
            .to_string(),
});

/// Zero-sized rule — the metadata is static, and the checker holds no
/// state between files.
#[derive(Default)]
pub struct AugroupClear;

impl LintRule for AugroupClear {
    fn id(&self) -> &RuleId {
        &ID
    }

    fn severity(&self) -> Severity {
        Severity::ShouldFix
    }

    fn description(&self) -> &str {
        "vim.api.nvim_create_augroup must be called with `{ clear = true }` so re-sourcing \
        the file does not append duplicate autocmds"
    }

    fn fix_guidance(&self) -> &FixGuidance {
        &FIX
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Finding> {
        let mut findings = Vec::new();
        for node in all_nodes(ctx.tree.tree()) {
            if node.kind() != "function_call" {
                continue;
            }
            let Some(name_node) = name_of_call(node) else {
                continue;
            };
            let name = ctx.text(ByteSpan::new(name_node.start_byte(), name_node.end_byte()));
            if !is_nvim_create_augroup(name) {
                continue;
            }
            let Some(args_node) = arguments_of_call(node) else {
                continue;
            };

            if !second_arg_has_clear_true(args_node, ctx) {
                findings.push(
                    ctx.finding(
                        self,
                        node,
                        "vim.api.nvim_create_augroup called without `{ clear = true }`",
                        "re-sourcing this file appends autocmds to the group instead of \
                        replacing them",
                    )
                    .fix("pass `{ clear = true }` as the second argument")
                    .build(),
                );
            }
        }
        findings
    }
}

fn name_of_call(call: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = call.walk();
    call.children(&mut cursor).next()
}

fn arguments_of_call(call: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = call.walk();
    call.children(&mut cursor).find(|n| n.kind() == "arguments")
}

fn is_nvim_create_augroup(text: &str) -> bool {
    // Strict qualified form only. Un-qualified `nvim_create_augroup`
    // is possible via `local nca = vim.api.nvim_create_augroup` — that
    // aliasing pattern is out of scope for v0.1.0.
    text.trim() == "vim.api.nvim_create_augroup"
}

fn second_arg_has_clear_true(args: Node<'_>, ctx: &LintContext<'_>) -> bool {
    let Some(table) = second_arg_table(args) else {
        return false;
    };
    table_has_clear_true(table, ctx)
}

fn second_arg_table<'tree>(args: Node<'tree>) -> Option<Node<'tree>> {
    // arguments = "(" arg ("," arg)* ")"
    // Walk arguments' children, skip punctuation, take the second real arg.
    let mut real_args = 0usize;
    let mut cursor = args.walk();
    let mut found_table = None;
    for child in args.children(&mut cursor) {
        match child.kind() {
            "(" | ")" | "," => continue,
            _ => {
                real_args += 1;
                if real_args == 2 && child.kind() == "table_constructor" {
                    found_table = Some(child);
                    break;
                }
                if real_args >= 2 {
                    break;
                }
            }
        }
    }
    found_table
}

fn table_has_clear_true(table: Node<'_>, ctx: &LintContext<'_>) -> bool {
    let mut cursor = table.walk();
    for child in table.children(&mut cursor) {
        if child.kind() != "field" {
            continue;
        }
        if field_matches_clear_true(child, ctx) {
            return true;
        }
    }
    false
}

fn field_matches_clear_true(field: Node<'_>, ctx: &LintContext<'_>) -> bool {
    // field = identifier "=" expression   (for "name = value" form)
    // Older / positional fields look different but are irrelevant here.
    let mut cursor = field.walk();
    let children: Vec<Node<'_>> = field.children(&mut cursor).collect();
    if children.len() < 3 {
        return false;
    }
    let name_node = children[0];
    let value_node = children[2];
    let name = ctx.text(ByteSpan::new(name_node.start_byte(), name_node.end_byte()));
    if name.trim() != "clear" {
        return false;
    }
    let value = ctx.text(ByteSpan::new(
        value_node.start_byte(),
        value_node.end_byte(),
    ));
    value.trim() == "true"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::parser::LuaParser;
    use crate::adapters::repo::LuaFileRole;
    use std::path::Path;

    fn load_fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("rules")
            .join("nvim-augroup-clear")
            .join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
    }

    fn run_rule(source: &str, relative: &Path) -> Vec<Finding> {
        let mut parser = LuaParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let role = LuaFileRole::Plugin;
        let ctx = LintContext {
            tree: &tree,
            source,
            role: &role,
            relative_path: relative,
        };
        AugroupClear.check(&ctx)
    }

    #[test]
    fn fires_on_positive_fixture() {
        let src = load_fixture("positive.lua");
        let findings = run_rule(&src, Path::new("plugin/x.lua"));
        assert_eq!(
            findings.len(),
            4,
            "positive fixture should trip the rule 4 times, got {}",
            findings.len()
        );
        // Snapshot only the wire-relevant fields so refactors to the
        // fixture text don't churn the snapshot.
        let stable: Vec<_> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "rule": f.rule.as_str(),
                    "severity": f.severity,
                    "line": f.location.line,
                    "message": f.message,
                })
            })
            .collect();
        insta::assert_json_snapshot!("nvim_augroup_clear_positive", stable);
    }

    #[test]
    fn does_not_fire_on_negative_fixture() {
        let src = load_fixture("negative.lua");
        let findings = run_rule(&src, Path::new("plugin/x.lua"));
        assert!(
            findings.is_empty(),
            "negative fixture unexpectedly tripped rule: {findings:#?}"
        );
    }

    #[test]
    fn metadata_matches_ontology() {
        let rule = AugroupClear;
        assert_eq!(rule.id().as_str(), "nvim/augroup-clear");
        assert_eq!(rule.id().category(), "nvim");
        assert_eq!(rule.severity(), Severity::ShouldFix);
        assert!(!rule.fix_guidance().is_auto_fixable());
    }
}
