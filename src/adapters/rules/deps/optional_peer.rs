//! `deps/optional-peer` — requires of optional peer plugins must
//! be guarded by `pcall`.
//!
//! Source pattern: the Lua-era equivalent of tpope's `silent!` idiom.
//! A plugin that lists another plugin as an *optional* integration
//! (not a hard runtime dep) must degrade silently when the peer is
//! missing. Bare `require("<peer>")` throws — and the throw propagates
//! all the way out of the plugin bootstrap, taking the whole plugin
//! down instead of just disabling the integration.
//!
//! Detection: walk every `function_call` whose function-name is the
//! identifier `require`, and:
//! - Skip if the target starts with `vim` or `vim.` (already available).
//! - Skip if the target starts with `plenary` or `plenary.` (universal
//!   ecosystem dep per the TODO exemption list).
//! - Skip if the target starts with the current file's own module name
//!   (first-party — always available because we're inside it).
//! - Skip if any ancestor within [`MAX_ANCESTOR_DEPTH`] levels is a
//!   `pcall` / `xpcall` function_call.
//! - Otherwise, fire.
//!
//! Exemptions do NOT cover `require("<some_stdlib>")` style calls to
//! Lua-standard modules (`string`, `math`, etc.) because in Neovim those
//! are already globals — you would not `require` them. If someone does,
//! the rule fires and that is the correct outcome.
//!
//! Auto-fix: not implemented in v0.1.0. The `pcall(require, "X")` form
//! changes the caller's variable binding pattern (`local m = ...` becomes
//! `local ok, m = ...`), which is context-dependent to rewrite safely.

use std::sync::LazyLock;

use tree_sitter::Node;

use crate::adapters::parser::all_nodes;
use crate::adapters::repo::LuaFileRole;
use crate::domain::finding::{ByteSpan, Finding};
use crate::domain::rule::{FixGuidance, RuleId};
use crate::domain::rule_engine::{LintContext, LintRule};
use crate::domain::severity::Severity;

static ID: LazyLock<RuleId> = LazyLock::new(|| RuleId::parse("deps/optional-peer").unwrap());

static FIX: LazyLock<FixGuidance> = LazyLock::new(|| FixGuidance::Manual {
    description: "wrap the require in `pcall(require, \"<name>\")` and branch on the ok flag \
        so the plugin degrades silently when the peer is missing"
        .to_string(),
});

/// Bounded ancestor walk when checking for a `pcall`/`xpcall` wrapper.
/// Ten levels is enough to cross a `pcall(function() ... require(...) ... end)`
/// idiom (which needs ~7) with headroom, without matching a `pcall`
/// that is so distant it does not actually protect this require.
const MAX_ANCESTOR_DEPTH: usize = 10;

#[derive(Default)]
pub struct OptionalPeer;

impl LintRule for OptionalPeer {
    fn id(&self) -> &RuleId {
        &ID
    }

    fn severity(&self) -> Severity {
        Severity::ShouldFix
    }

    fn description(&self) -> &str {
        "require calls for optional peer plugins must be wrapped in pcall so the plugin \
        degrades silently when the peer is absent"
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
            let Some(name_node) = first_child(node) else {
                continue;
            };
            if name_node.kind() != "identifier" {
                continue;
            }
            let name = ctx.text(ByteSpan::new(name_node.start_byte(), name_node.end_byte()));
            if name.trim() != "require" {
                continue;
            }
            let Some(args) = child_of_kind(node, "arguments") else {
                continue;
            };
            let Some(target) = string_arg_content(args, ctx) else {
                continue;
            };
            if is_exempt(&target, ctx) {
                continue;
            }
            if is_pcall_wrapped(node, ctx) {
                continue;
            }
            findings.push(
                ctx.finding(
                    self,
                    node,
                    format!("`require(\"{target}\")` is not guarded by pcall"),
                    "an optional peer must degrade silently — an unguarded require throws \
                    and takes the whole plugin down when the peer is absent",
                )
                .fix(format!(
                    "rewrite as `local ok, m = pcall(require, \"{target}\")` and branch on `ok`"
                ))
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

/// Extract the first `string_content` inside `arguments`. Returns `None`
/// if the first positional arg isn't a string literal (e.g.
/// `require(mod_name)` with a variable target — those we cannot resolve
/// statically and deliberately skip).
fn string_arg_content(args: Node<'_>, ctx: &LintContext<'_>) -> Option<String> {
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        match child.kind() {
            "(" | ")" | "," => continue,
            "string" => {
                let content = child_of_kind(child, "string_content")?;
                let text = ctx.text(ByteSpan::new(content.start_byte(), content.end_byte()));
                return Some(text.to_string());
            }
            _ => return None,
        }
    }
    None
}

fn is_exempt(target: &str, ctx: &LintContext<'_>) -> bool {
    if target == "vim" || target.starts_with("vim.") {
        return true;
    }
    if target == "plenary" || target.starts_with("plenary.") {
        return true;
    }
    if let Some(module) = self_module(ctx)
        && (target == module || target.starts_with(&format!("{module}.")))
    {
        return true;
    }
    false
}

fn self_module<'a>(ctx: &'a LintContext<'_>) -> Option<&'a str> {
    match ctx.role {
        LuaFileRole::LuaInit { module }
        | LuaFileRole::LuaHealth { module }
        | LuaFileRole::Lua { module } => Some(module.as_str()),
        _ => ctx.primary_module,
    }
}

fn is_pcall_wrapped(node: Node<'_>, ctx: &LintContext<'_>) -> bool {
    let mut current = node.parent();
    for _ in 0..MAX_ANCESTOR_DEPTH {
        let Some(n) = current else {
            return false;
        };
        if n.kind() == "function_call"
            && let Some(name_node) = first_child(n)
            && name_node.kind() == "identifier"
        {
            let text = ctx.text(ByteSpan::new(name_node.start_byte(), name_node.end_byte()));
            if matches!(text.trim(), "pcall" | "xpcall") {
                return true;
            }
        }
        current = n.parent();
    }
    false
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
            .join("deps-optional-peer")
            .join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
    }

    fn run_rule(source: &str, role: LuaFileRole, primary_module: Option<&str>) -> Vec<Finding> {
        let mut parser = LuaParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let rel = Path::new("plugin/x.lua");
        let ctx = LintContext {
            tree: &tree,
            source,
            role: &role,
            relative_path: rel,
            primary_module,
        };
        OptionalPeer.check(&ctx)
    }

    #[test]
    fn fires_on_positive_fixture() {
        let src = load_fixture("positive.lua");
        // The negative fixture references `myplugin` as first-party;
        // the positive fixture does not — no first-party context needed.
        let findings = run_rule(&src, LuaFileRole::Plugin, None);
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
        insta::assert_json_snapshot!("deps_optional_peer_positive", stable);
    }

    #[test]
    fn does_not_fire_on_negative_fixture() {
        let src = load_fixture("negative.lua");
        // Negative fixture treats `myplugin` as first-party — set it as
        // the primary module so that exemption engages.
        let findings = run_rule(&src, LuaFileRole::Plugin, Some("myplugin"));
        assert!(
            findings.is_empty(),
            "negative fixture unexpectedly tripped rule: {findings:#?}"
        );
    }

    #[test]
    fn first_party_exempt_via_ctx_role_module() {
        let src = "local util = require(\"foo.util\")\n";
        // File is `lua/foo/init.lua` — role carries module="foo".
        let findings = run_rule(
            src,
            LuaFileRole::LuaInit {
                module: "foo".to_string(),
            },
            None,
        );
        assert!(findings.is_empty(), "first-party require should be exempt");
    }

    #[test]
    fn variable_arg_is_skipped() {
        // require(module_var) can't be resolved statically — skip.
        let src = "local m = require(module_var)\n";
        let findings = run_rule(src, LuaFileRole::Plugin, None);
        assert!(
            findings.is_empty(),
            "variable-target require should be silently skipped"
        );
    }

    #[test]
    fn metadata_matches_ontology() {
        let rule = OptionalPeer;
        assert_eq!(rule.id().as_str(), "deps/optional-peer");
        assert_eq!(rule.id().category(), "deps");
        assert_eq!(rule.severity(), Severity::ShouldFix);
        assert!(!rule.fix_guidance().is_auto_fixable());
    }
}
