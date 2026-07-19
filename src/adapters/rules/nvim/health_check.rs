//! `nvim/health-check` — every plugin repo must ship
//! `lua/<name>/health.lua`.
//!
//! Source pattern: tj's universal convention, adopted across the modern
//! Neovim ecosystem. `:checkhealth <name>` is the entry point users run
//! when a plugin misbehaves — no health module means the user's first
//! debugging step returns "no healthcheck for `<name>`" and they cannot
//! self-diagnose (missing external deps, wrong Neovim version, absent
//! peer plugins, permission issues, etc.). The gap is universal in the
//! jedi-knights suite per the earlier audit — this rule locks it.
//!
//! Detection is repo-level: fires from [`LintRule::check_repo`], not
//! per-file. A "plugin repo" is any directory that has either a
//! `plugin/*.lua` file (declared plugin surface) or a `lua/<name>/init.lua`
//! (module entrypoint). If the primary module has no matching
//! `lua/<name>/health.lua`, fire — pointing at the module's init.lua
//! (or the first `plugin/` file when there is no init) so the fix is
//! anchored where the reader will look.
//!
//! Auto-fix: not implemented in v0.1.0. Scaffolding a `health.lua`
//! meaningfully requires knowing what to check for — the rule only
//! signals that one is missing. A companion `plug-scaffold` step
//! (portfolio TODO PS-N) can fill this in.

use std::sync::LazyLock;

use crate::adapters::repo::LuaFileRole;
use crate::domain::finding::Finding;
use crate::domain::rule::{FixGuidance, RuleId};
use crate::domain::rule_engine::{LintRule, RepoContext};
use crate::domain::severity::Severity;

static ID: LazyLock<RuleId> = LazyLock::new(|| RuleId::parse("nvim/health-check").unwrap());

static FIX: LazyLock<FixGuidance> = LazyLock::new(|| FixGuidance::Manual {
    description: "add `lua/<name>/health.lua` exporting a `check()` function that reports \
        external-dep availability, peer-plugin presence, and any other diagnostics a user \
        would need to self-debug"
        .to_string(),
});

#[derive(Default)]
pub struct HealthCheck;

impl LintRule for HealthCheck {
    fn id(&self) -> &RuleId {
        &ID
    }

    fn severity(&self) -> Severity {
        Severity::MustFix
    }

    fn description(&self) -> &str {
        "every plugin repo must ship `lua/<name>/health.lua` so `:checkhealth <name>` \
        returns actionable diagnostics instead of a stub"
    }

    fn fix_guidance(&self) -> &FixGuidance {
        &FIX
    }

    fn check_repo(&self, ctx: &RepoContext<'_>) -> Vec<Finding> {
        // Only fire on repos that actually look like plugins — either a
        // plugin/ file or a lua/<name>/init.lua entry. A pure library or
        // docs repo does not need :checkhealth support.
        let has_plugin_surface = ctx.any_role(|r| matches!(r, LuaFileRole::Plugin))
            || ctx.any_role(|r| matches!(r, LuaFileRole::LuaInit { .. }));
        if !has_plugin_surface {
            return Vec::new();
        }

        // Determine the primary module. If there's no LuaInit we cannot
        // reliably name the healthcheck path — skip. plugin/-only repos
        // without a paired module are unusual (the plugin/ file itself
        // would typically `require("<name>")`); the rule prefers to
        // stay silent rather than emit a low-confidence finding.
        let Some(module) = ctx.primary_module() else {
            return Vec::new();
        };

        let has_health = ctx.files().iter().any(|f| match &f.role {
            LuaFileRole::LuaHealth { module: m } => m == module,
            _ => false,
        });
        if has_health {
            return Vec::new();
        }

        // Anchor the finding at the module's init.lua — the file the
        // reader will open when they read "add lua/<name>/health.lua".
        let anchor = ctx
            .files()
            .iter()
            .find(|f| matches!(&f.role, LuaFileRole::LuaInit { module: m } if m == module))
            .map(|f| f.relative_path.clone())
            .unwrap_or_else(|| std::path::PathBuf::from(format!("lua/{module}/init.lua")));

        vec![
            ctx.finding_at_file(
                self,
                anchor,
                format!("plugin repo `{module}` is missing `lua/{module}/health.lua`"),
                "users running `:checkhealth` cannot self-diagnose missing deps, wrong \
                Neovim version, or absent peer plugins without a healthcheck module",
            )
            .fix(format!(
                "create `lua/{module}/health.lua` with a `check()` function reporting \
                external-dep availability, peer-plugin presence, and version constraints"
            ))
            .build(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::repo::LuaFile;
    use std::path::PathBuf;

    fn file(relative: &str, role: LuaFileRole) -> LuaFile {
        LuaFile {
            path: PathBuf::from("/tmp/repo").join(relative),
            relative_path: PathBuf::from(relative),
            role,
        }
    }

    fn run_repo(files: Vec<LuaFile>) -> Vec<Finding> {
        let root = PathBuf::from("/tmp/repo");
        let ctx = RepoContext {
            root: &root,
            files: &files,
        };
        HealthCheck.check_repo(&ctx)
    }

    #[test]
    fn fires_when_plugin_repo_missing_health() {
        let files = vec![
            file("plugin/foo.lua", LuaFileRole::Plugin),
            file(
                "lua/foo/init.lua",
                LuaFileRole::LuaInit {
                    module: "foo".to_string(),
                },
            ),
        ];
        let findings = run_repo(files);
        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert_eq!(f.rule.as_str(), "nvim/health-check");
        assert_eq!(f.severity, Severity::MustFix);
        assert_eq!(f.location.file, PathBuf::from("lua/foo/init.lua"));
        assert!(f.message.contains("missing `lua/foo/health.lua`"));

        let stable = serde_json::json!({
            "rule": f.rule.as_str(),
            "severity": f.severity,
            "anchor": f.location.file.to_string_lossy(),
            "message": f.message,
        });
        insta::assert_json_snapshot!("nvim_health_check_missing", stable);
    }

    #[test]
    fn does_not_fire_when_health_present() {
        let files = vec![
            file("plugin/foo.lua", LuaFileRole::Plugin),
            file(
                "lua/foo/init.lua",
                LuaFileRole::LuaInit {
                    module: "foo".to_string(),
                },
            ),
            file(
                "lua/foo/health.lua",
                LuaFileRole::LuaHealth {
                    module: "foo".to_string(),
                },
            ),
        ];
        let findings = run_repo(files);
        assert!(findings.is_empty());
    }

    #[test]
    fn does_not_fire_for_non_plugin_repo() {
        // A repo with no plugin/ and no lua/<name>/init.lua isn't a
        // plugin repo; it's docs or a library shell. Skip.
        let files = vec![file("tests/spec.lua", LuaFileRole::Test)];
        let findings = run_repo(files);
        assert!(findings.is_empty());
    }

    #[test]
    fn does_not_fire_when_health_matches_module() {
        // Health for module "bar" doesn't satisfy module "foo".
        let files = vec![
            file(
                "lua/foo/init.lua",
                LuaFileRole::LuaInit {
                    module: "foo".to_string(),
                },
            ),
            file(
                "lua/bar/health.lua",
                LuaFileRole::LuaHealth {
                    module: "bar".to_string(),
                },
            ),
        ];
        let findings = run_repo(files);
        assert_eq!(findings.len(), 1, "bar's health should not exempt foo");
    }

    #[test]
    fn skips_plugin_only_repo_without_module() {
        // A plugin/-only repo (unusual) without a lua/<name>/init.lua
        // has no reliable module name to point at — stay silent.
        let files = vec![file("plugin/foo.lua", LuaFileRole::Plugin)];
        let findings = run_repo(files);
        assert!(findings.is_empty());
    }

    #[test]
    fn fires_from_registered_engine() {
        use crate::adapters::rules::built_in_rules;
        use crate::domain::rule_engine::RuleEngine;

        let engine = RuleEngine::new(built_in_rules());
        let files = vec![
            file("plugin/foo.lua", LuaFileRole::Plugin),
            file(
                "lua/foo/init.lua",
                LuaFileRole::LuaInit {
                    module: "foo".to_string(),
                },
            ),
        ];
        let root = PathBuf::from("/tmp/repo");
        let ctx = RepoContext {
            root: &root,
            files: &files,
        };
        let findings = engine.check_repo(&ctx);
        assert!(
            findings
                .iter()
                .any(|f| f.rule.as_str() == "nvim/health-check"),
            "engine did not surface health-check finding: {findings:#?}"
        );
    }

    #[test]
    fn metadata_matches_ontology() {
        let rule = HealthCheck;
        assert_eq!(rule.id().as_str(), "nvim/health-check");
        assert_eq!(rule.id().category(), "nvim");
        assert_eq!(rule.severity(), Severity::MustFix);
        assert!(!rule.fix_guidance().is_auto_fixable());
    }
}
