//! `nvim/setup-optional` — commands defined in `plugin/*.lua` must work
//! without `setup()` having been called first.
//!
//! Source pattern: tpope's setup-optional invariant, ported to the
//! `plugin/*.lua` era. A well-behaved plugin exposes a `setup()` for
//! *configuration*, not for *installation*. The user's commands
//! (`:MyPluginFoo`) must exist and behave sensibly whether or not
//! `setup()` ever ran — a common failure mode is a `plugin/` command
//! that reaches into `require("<name>").config.field`, which is `nil`
//! until `setup()` mutates the module.
//!
//! Detection is a heuristic — the `plugin/` file that touches
//! `require("<X>").config` is the tell. If the field is legitimately
//! defaulted at the top of the module (`M.config = { ... }`) then the
//! access is benign, but the rule cannot distinguish this from unsafe
//! access without cross-file semantic analysis. False positives are
//! acceptable at Must Fix here — the fix (lazy access + a `setup()`
//! guard) is small, and the failure mode being prevented (nil-index at
//! command invocation) is opaque to users.
//!
//! Match:
//! - Only [`LuaFileRole::Plugin`] files.
//! - Any `dot_index_expression` whose LHS is a `function_call` to
//!   `require` and whose field identifier is `config`.
//! - The rule fires on the *innermost* `require(...).config` link, so
//!   `require("m").config.a.b.c` produces exactly one finding.
//!
//! Known false positives (v0.1.0 scope):
//! - `require("plenary").config` and similar universal-peer targets.
//!   Exempting these would require a target-based allow list; deferred.
//! - Access inside a `pcall` / `setup`-guarded branch — the rule does
//!   not walk ancestors to look for an `if <ok>` guard.
//!
//! Auto-fix: not implemented. The correct rewrite depends on whether
//! the plugin ships a default-config module (access can stay eager but
//! reads from a static table) or whether the command should assert
//! setup ran first — heuristic fix guidance only.

use std::sync::LazyLock;

use tree_sitter::Node;

use crate::adapters::parser::all_nodes;
use crate::adapters::repo::LuaFileRole;
use crate::domain::finding::{ByteSpan, Finding};
use crate::domain::rule::{FixGuidance, RuleId};
use crate::domain::rule_engine::{LintContext, LintRule};
use crate::domain::severity::Severity;

static ID: LazyLock<RuleId> = LazyLock::new(|| RuleId::parse("nvim/setup-optional").unwrap());

static FIX: LazyLock<FixGuidance> = LazyLock::new(|| FixGuidance::Manual {
    description: "either default the config field at module top-level (so it is non-nil \
        without setup running) or assert `setup()` ran before reading it — for command \
        callbacks, read the field lazily inside the callback and branch on nil"
        .to_string(),
});

#[derive(Default)]
pub struct SetupOptional;

impl LintRule for SetupOptional {
    fn id(&self) -> &RuleId {
        &ID
    }

    fn severity(&self) -> Severity {
        Severity::MustFix
    }

    fn description(&self) -> &str {
        "commands defined in `plugin/*.lua` must not depend on `setup()` having run — \
        reaching into `require(\"<name>\").config` is the tell"
    }

    fn fix_guidance(&self) -> &FixGuidance {
        &FIX
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Finding> {
        if !matches!(ctx.role, LuaFileRole::Plugin) {
            return Vec::new();
        }

        let mut findings = Vec::new();
        for node in all_nodes(ctx.tree.tree()) {
            if node.kind() != "dot_index_expression" {
                continue;
            }
            let Some((lhs, field)) = split_dot_index(node) else {
                continue;
            };
            if field.kind() != "identifier" {
                continue;
            }
            let field_name = ctx.text(ByteSpan::new(field.start_byte(), field.end_byte()));
            if field_name.trim() != "config" {
                continue;
            }
            let Some(target) = require_target(lhs, ctx) else {
                continue;
            };
            findings.push(
                ctx.finding(
                    self,
                    node,
                    format!(
                        "`require(\"{target}\").config` accessed in `plugin/` — command \
                        surface must not depend on `setup()` having run"
                    ),
                    "if the user runs `:MyPluginFoo` before their config's `setup()` \
                    line executes, this nil-indexes and the command fails opaquely",
                )
                .fix(
                    "default the config table at module top-level, or read the field lazily \
                        inside the command callback and branch on nil",
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

/// Split a `dot_index_expression` into (lhs, field). Returns `None` if
/// the node's shape does not match the standard 3-child form
/// `lhs "." field`.
fn split_dot_index<'t>(node: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
    let mut cursor = node.walk();
    let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
    if children.len() < 3 {
        return None;
    }
    Some((children[0], children[2]))
}

/// If `lhs` is a `function_call` to `require`, extract the string
/// literal argument as the target module name. Returns `None`
/// otherwise — we deliberately skip dynamic require targets.
fn require_target(lhs: Node<'_>, ctx: &LintContext<'_>) -> Option<String> {
    if lhs.kind() != "function_call" {
        return None;
    }
    let func_name = first_child(lhs)?;
    if func_name.kind() != "identifier" {
        return None;
    }
    let name = ctx.text(ByteSpan::new(func_name.start_byte(), func_name.end_byte()));
    if name.trim() != "require" {
        return None;
    }
    let args = child_of_kind(lhs, "arguments")?;
    let mut cursor = args.walk();
    for arg in args.children(&mut cursor) {
        match arg.kind() {
            "(" | ")" | "," => continue,
            "string" => {
                let content = child_of_kind(arg, "string_content")?;
                return Some(
                    ctx.text(ByteSpan::new(content.start_byte(), content.end_byte()))
                        .to_string(),
                );
            }
            _ => return None,
        }
    }
    None
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
            .join("nvim-setup-optional")
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
        SetupOptional.check(&ctx)
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
        insta::assert_json_snapshot!("nvim_setup_optional_positive", stable);
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
        // Same pattern in lua/<name>/init.lua — not the target of this rule.
        let src = r#"local cfg = require("myplugin").config"#;
        let findings = run_rule(
            src,
            LuaFileRole::LuaInit {
                module: "foo".to_string(),
            },
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn fires_once_on_deep_chain() {
        // require("m").config.a.b — fires once on the innermost link.
        let src = r#"local x = require("m").config.a.b"#;
        let findings = run_rule(src, LuaFileRole::Plugin);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_non_require_dot_index() {
        // Local `M.config` is a plain identifier-dot-config, not a
        // require chain.
        let src = "local M = {}\nM.config = { x = 1 }\nlocal cfg = M.config";
        let findings = run_rule(src, LuaFileRole::Plugin);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_setup_field() {
        // `require("m").setup(...)` is the intended entry point.
        let src = r#"require("myplugin").setup({})"#;
        let findings = run_rule(src, LuaFileRole::Plugin);
        assert!(findings.is_empty());
    }

    #[test]
    fn metadata_matches_ontology() {
        let rule = SetupOptional;
        assert_eq!(rule.id().as_str(), "nvim/setup-optional");
        assert_eq!(rule.id().category(), "nvim");
        assert_eq!(rule.severity(), Severity::MustFix);
        assert!(!rule.fix_guidance().is_auto_fixable());
    }
}
