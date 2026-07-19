//! `nvim/plug-mapping` — `plugin/*.lua` files must not ship default
//! `<leader>` / `<localleader>` keymaps.
//!
//! Source pattern: tpope's `<Plug>`-first idiom, ported into the
//! `vim.keymap.set` era. Shipping `vim.keymap.set(mode, "<leader>xx", ...)`
//! at plugin load time steals the leader-prefixed key from the user
//! and creates a conflict landscape between competing plugins. The
//! idiomatic pattern is to expose a `<Plug>(plugin-action)` mapping
//! from `plugin/` and document the recommended user binding in the
//! README, leaving the user in control of which key triggers which
//! action.
//!
//! Detection is per-file, scoped to [`LuaFileRole::Plugin`]. Every
//! `function_call` whose function-name is `vim.keymap.set` is
//! examined; if the second positional argument is a string literal
//! whose content starts (case-insensitively) with `<leader>` or
//! `<localleader>`, the rule fires. `<Plug>` LHSes and non-leader keys
//! are silent, as are dynamic (non-string) LHSes.
//!
//! The older `vim.api.nvim_set_keymap` API is deliberately out of
//! scope for v0.1.0 — the population using it is shrinking, and
//! extending the match would double the detection surface for
//! marginal signal.
//!
//! Auto-fix: not implemented. The `<Plug>` translation is
//! context-dependent (name of the plugin, name of the action, whether
//! the RHS is a Vim command or a Lua callback).

use std::sync::LazyLock;

use tree_sitter::Node;

use crate::adapters::parser::all_nodes;
use crate::adapters::repo::LuaFileRole;
use crate::domain::finding::{ByteSpan, Finding};
use crate::domain::rule::{FixGuidance, RuleId};
use crate::domain::rule_engine::{LintContext, LintRule};
use crate::domain::severity::Severity;

static ID: LazyLock<RuleId> = LazyLock::new(|| RuleId::parse("nvim/plug-mapping").unwrap());

static FIX: LazyLock<FixGuidance> = LazyLock::new(|| FixGuidance::Manual {
    description: "expose a `<Plug>(plugin-action)` mapping from `plugin/`, document the \
        recommended user binding in the README, and let the user remap it: \
        `vim.keymap.set(mode, \"<Plug>(myplugin-action)\", callback)`"
        .to_string(),
});

#[derive(Default)]
pub struct PlugMapping;

impl LintRule for PlugMapping {
    fn id(&self) -> &RuleId {
        &ID
    }

    fn severity(&self) -> Severity {
        Severity::ShouldFix
    }

    fn description(&self) -> &str {
        "plugin/ files must not ship default `<leader>` keymaps — expose `<Plug>` mappings \
        and let the user bind their own key"
    }

    fn fix_guidance(&self) -> &FixGuidance {
        &FIX
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Finding> {
        // Scoped to plugin/*.lua. Keymaps in lua/<name>/*.lua are the
        // caller's responsibility to set up — those aren't shipped by
        // default at load time.
        if !matches!(ctx.role, LuaFileRole::Plugin) {
            return Vec::new();
        }

        let mut findings = Vec::new();
        for node in all_nodes(ctx.tree.tree()) {
            if node.kind() != "function_call" {
                continue;
            }
            let Some(name_node) = first_child(node) else {
                continue;
            };
            let name = ctx.text(ByteSpan::new(name_node.start_byte(), name_node.end_byte()));
            if name.trim() != "vim.keymap.set" {
                continue;
            }
            let Some(args) = child_of_kind(node, "arguments") else {
                continue;
            };
            let Some(lhs) = nth_string_arg_content(args, 2, ctx) else {
                continue;
            };
            if !is_leader_default(&lhs) {
                continue;
            }
            findings.push(
                ctx.finding(
                    self,
                    node,
                    format!(
                        "`vim.keymap.set` in `plugin/` binds default `{lhs}` instead of a \
                        `<Plug>` indirection"
                    ),
                    "hardcoded `<leader>` bindings in plugin/ steal the key from the user \
                    and create conflicts when two plugins bind the same suffix",
                )
                .fix(
                    "expose `<Plug>(<name>-<action>)` from `plugin/` and let the user bind \
                        their own key in their config",
                )
                .build(),
            );
        }
        findings
    }
}

fn first_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).next()
}

fn child_of_kind<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|n| n.kind() == kind)
}

/// Extract the `n`th real positional argument's string content, if it
/// is a string literal. `n` is 1-indexed to read naturally at the call
/// site ("the 2nd arg"). Returns `None` when the arg is not a string
/// (dynamic / table / function).
fn nth_string_arg_content(args: Node<'_>, n: usize, ctx: &LintContext<'_>) -> Option<String> {
    let mut real = 0usize;
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        match child.kind() {
            "(" | ")" | "," => continue,
            _ => {
                real += 1;
                if real < n {
                    continue;
                }
                if child.kind() == "string" {
                    let content = child_of_kind(child, "string_content")?;
                    let text = ctx.text(ByteSpan::new(content.start_byte(), content.end_byte()));
                    return Some(text.to_string());
                }
                return None;
            }
        }
    }
    None
}

fn is_leader_default(lhs: &str) -> bool {
    // Case-insensitive match — Vim treats `<Leader>` and `<leader>` as
    // the same replacement token.
    let head_lower: String = lhs.chars().take(15).flat_map(char::to_lowercase).collect();
    head_lower.starts_with("<leader>") || head_lower.starts_with("<localleader>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::parser::LuaParser;
    use std::path::Path;

    fn load_fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("rules")
            .join("nvim-plug-mapping")
            .join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
    }

    fn run_rule(source: &str, role: LuaFileRole) -> Vec<Finding> {
        let mut parser = LuaParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let ctx = LintContext {
            tree: &tree,
            source,
            role: &role,
            relative_path: Path::new("plugin/x.lua"),
            primary_module: None,
        };
        PlugMapping.check(&ctx)
    }

    #[test]
    fn fires_on_positive_fixture() {
        let src = load_fixture("positive.lua");
        let findings = run_rule(&src, LuaFileRole::Plugin);
        assert_eq!(
            findings.len(),
            4,
            "positive fixture should trip the rule 4 times, got {}",
            findings.len()
        );
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
        insta::assert_json_snapshot!("nvim_plug_mapping_positive", stable);
    }

    #[test]
    fn does_not_fire_on_negative_fixture() {
        let src = load_fixture("negative.lua");
        let findings = run_rule(&src, LuaFileRole::Plugin);
        assert!(
            findings.is_empty(),
            "negative fixture unexpectedly tripped rule: {findings:#?}"
        );
    }

    #[test]
    fn does_not_fire_outside_plugin_dir() {
        // Same offending pattern in lua/<name>/init.lua — not the scope.
        let src = r#"vim.keymap.set("n", "<leader>ff", function() end)"#;
        let findings = run_rule(
            src,
            LuaFileRole::LuaInit {
                module: "foo".to_string(),
            },
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn case_insensitive_leader_match() {
        for lhs in ["<leader>x", "<Leader>x", "<LEADER>x"] {
            assert!(
                is_leader_default(lhs),
                "expected `{lhs}` to match as a leader default"
            );
        }
        for lhs in ["<localleader>x", "<LocalLeader>x", "<LOCALLEADER>x"] {
            assert!(
                is_leader_default(lhs),
                "expected `{lhs}` to match as a localleader default"
            );
        }
    }

    #[test]
    fn plug_lhs_not_matched() {
        assert!(!is_leader_default("<Plug>(myplugin-x)"));
        assert!(!is_leader_default("<C-p>"));
        assert!(!is_leader_default("gd"));
    }

    #[test]
    fn metadata_matches_ontology() {
        let rule = PlugMapping;
        assert_eq!(rule.id().as_str(), "nvim/plug-mapping");
        assert_eq!(rule.id().category(), "nvim");
        assert_eq!(rule.severity(), Severity::ShouldFix);
        assert!(!rule.fix_guidance().is_auto_fixable());
    }
}
