//! Rule engine — the port trait ([`LintRule`]), its runtime context
//! ([`LintContext`]), and the driver that fans a context out over every
//! registered rule ([`RuleEngine`]).
//!
//! **Hexagonal-purity note.** [`LintContext`] holds references to
//! [`ParsedTree`][crate::adapters::parser::ParsedTree] and
//! [`LuaFileRole`][crate::adapters::repo::LuaFileRole], both of which
//! live in the adapters layer. Strict hexagonal architecture would put
//! a port trait between them; we do not, because there is exactly one
//! parser (tree-sitter Lua) and exactly one file classifier and rules
//! genuinely need access to the raw AST. Introducing an abstract
//! `AstView` trait would mean re-declaring tree-sitter's `Node` API in
//! trait form — pure overhead per the design-patterns over-engineering
//! guard. The tradeoff is that swapping parsers would require touching
//! the rule engine; that is acceptable for a Lua-only linter.

use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::adapters::parser::ParsedTree;
use crate::adapters::repo::{LuaFile, LuaFileRole};
use crate::domain::finding::{ByteSpan, Finding, Location};
use crate::domain::rule::{FixGuidance, RuleId};
use crate::domain::severity::Severity;

/// Static metadata that every rule exposes to the engine — the shape
/// callers need to disable / re-severity a rule via config without
/// invoking `check`.
pub trait LintRule: Send + Sync {
    fn id(&self) -> &RuleId;
    fn severity(&self) -> Severity;
    fn description(&self) -> &str;
    fn fix_guidance(&self) -> &FixGuidance;

    /// Run the rule against a single file's context and return every
    /// violation. May return an empty `Vec`; must never panic on
    /// well-formed input. Default no-op so repo-level rules can skip.
    fn check(&self, _ctx: &LintContext<'_>) -> Vec<Finding> {
        Vec::new()
    }

    /// Run the rule against the whole-repo context after all files
    /// have been walked. Fires exactly once per repo — used for
    /// presence/absence rules like `nvim/health-check` that can't be
    /// decided from a single file. Default no-op so per-file rules
    /// can skip.
    fn check_repo(&self, _ctx: &RepoContext<'_>) -> Vec<Finding> {
        Vec::new()
    }
}

/// Everything a rule needs to inspect one file.
///
/// Constructed by the CLI driver (see PA-6) — parse the source, look
/// up the classification, and pass everything by reference so no
/// per-rule allocation happens for the context itself.
pub struct LintContext<'a> {
    pub tree: &'a ParsedTree,
    pub source: &'a str,
    pub role: &'a LuaFileRole,
    pub relative_path: &'a Path,
}

impl<'a> LintContext<'a> {
    /// Build a Location for `node` anchored on the file this context
    /// wraps. Line and column are 1-indexed (tree-sitter reports 0-indexed).
    pub fn locate(&self, node: Node<'_>) -> Location {
        let start = node.start_position();
        Location {
            file: self.relative_path.to_path_buf(),
            line: (start.row + 1) as u32,
            column: (start.column + 1) as u32,
            byte_span: ByteSpan::new(node.start_byte(), node.end_byte()),
        }
    }

    /// Start building a finding attached to `node` for `rule`. Fill in
    /// `message` and `why`; call [`FindingBuilder::fix`] before
    /// [`FindingBuilder::build`] for Must Fix / Should Fix findings.
    pub fn finding(
        &self,
        rule: &dyn LintRule,
        node: Node<'_>,
        message: impl Into<String>,
        why: impl Into<String>,
    ) -> FindingBuilder {
        FindingBuilder {
            rule_id: rule.id().clone(),
            severity: rule.severity(),
            location: self.locate(node),
            message: message.into(),
            why: why.into(),
            fix: None,
        }
    }

    /// Slice of `source` matching a byte span. Handy for rules that
    /// need to inspect a node's text without re-hashing it.
    pub fn text(&self, span: ByteSpan) -> &'a str {
        &self.source[span.start..span.end]
    }
}

/// Whole-repo context passed to [`LintRule::check_repo`]. Provides
/// enough information for rules that assert repo-shape invariants
/// (e.g. "a plugin repo must ship `lua/<name>/health.lua`") without
/// needing to open the filesystem again.
pub struct RepoContext<'a> {
    pub root: &'a Path,
    pub files: &'a [LuaFile],
}

impl<'a> RepoContext<'a> {
    /// The "primary" module of this repo — the module name shared by
    /// the first `lua/<X>.lua` or `lua/<X>/init.lua` file we find,
    /// after sorting by relative path. `None` if there's no LuaInit
    /// entry at all (e.g. a plugin/-only repo).
    pub fn primary_module(&self) -> Option<&str> {
        self.files.iter().find_map(|f| match &f.role {
            LuaFileRole::LuaInit { module } => Some(module.as_str()),
            _ => None,
        })
    }

    /// True if any file's role satisfies `pred`. Handy for rules like
    /// "this repo has plugin/ files, so it must also ship a healthcheck".
    pub fn any_role<F: Fn(&LuaFileRole) -> bool>(&self, pred: F) -> bool {
        self.files.iter().any(|f| pred(&f.role))
    }

    /// Iterate every LuaFile — for rules that scan the discovery
    /// output directly.
    pub fn files(&self) -> &[LuaFile] {
        self.files
    }

    /// Build a Finding anchored at a specific file (line 1, col 1).
    /// Used for repo-level findings that don't have a natural node
    /// anchor — the file we point at is the actionable location for
    /// the fix, not the file that "caused" the finding.
    pub fn finding_at_file(
        &self,
        rule: &dyn LintRule,
        at: impl Into<PathBuf>,
        message: impl Into<String>,
        why: impl Into<String>,
    ) -> FindingBuilder {
        FindingBuilder {
            rule_id: rule.id().clone(),
            severity: rule.severity(),
            location: Location {
                file: at.into(),
                line: 1,
                column: 1,
                byte_span: ByteSpan::new(0, 0),
            },
            message: message.into(),
            why: why.into(),
            fix: None,
        }
    }
}

/// Fluent Finding constructor. Owned rather than borrowing from the
/// context so it can be stored and returned without lifetime drama.
pub struct FindingBuilder {
    rule_id: RuleId,
    severity: Severity,
    location: Location,
    message: String,
    why: String,
    fix: Option<String>,
}

impl FindingBuilder {
    pub fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    pub fn build(self) -> Finding {
        Finding {
            rule: self.rule_id,
            severity: self.severity,
            location: self.location,
            message: self.message,
            why: self.why,
            fix: self.fix,
        }
    }
}

/// Registry of rules the engine will apply. Rules are held behind
/// `Box<dyn LintRule>` so the registry can mix zero-sized rules and
/// stateful rules uniformly.
pub struct RuleEngine {
    rules: Vec<Box<dyn LintRule>>,
}

impl RuleEngine {
    pub fn new(rules: Vec<Box<dyn LintRule>>) -> Self {
        Self { rules }
    }

    /// Run every registered rule against `ctx`. Findings are returned
    /// in rule-registration order; the reporter is responsible for
    /// bucket-sorting by severity per `rules/findings-format.md`.
    pub fn check(&self, ctx: &LintContext<'_>) -> Vec<Finding> {
        let mut findings = Vec::new();
        for rule in &self.rules {
            findings.extend(rule.check(ctx));
        }
        findings
    }

    /// Run every registered rule against the whole-repo `ctx`. Called
    /// once after all per-file walks complete.
    pub fn check_repo(&self, ctx: &RepoContext<'_>) -> Vec<Finding> {
        let mut findings = Vec::new();
        for rule in &self.rules {
            findings.extend(rule.check_repo(ctx));
        }
        findings
    }

    /// Read-only access to the registered rules — used by the CLI's
    /// `--list-rules` output and by config validation.
    pub fn rules(&self) -> &[Box<dyn LintRule>] {
        &self.rules
    }

    /// True when the registry contains a rule with the given ID.
    /// Used by config validation to reject typos in enable/disable
    /// lists before the walk starts.
    pub fn has_rule(&self, id: &RuleId) -> bool {
        self.rules.iter().any(|r| r.id() == id)
    }
}

impl Default for RuleEngine {
    /// Empty engine — useful in tests. Production callers construct
    /// via [`RuleEngine::new`] with [`crate::adapters::rules::built_in_rules`].
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// Convenience for tests and the CLI: what to feed a rule against a
/// snapshot of a full repo walk. Not required by the trait.
#[derive(Debug)]
pub struct FileContext {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub source: String,
    pub role: LuaFileRole,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::parser::LuaParser;

    // A minimal test rule that fires on every `chunk` node — enough to
    // exercise LintContext, FindingBuilder, and RuleEngine without
    // waiting for PA-5.
    struct AlwaysFire {
        id: RuleId,
        fix: FixGuidance,
    }

    impl AlwaysFire {
        fn new() -> Self {
            Self {
                id: RuleId::parse("nvim/health-check").unwrap(),
                fix: FixGuidance::Manual {
                    description: "test-only rule".to_string(),
                },
            }
        }
    }

    impl LintRule for AlwaysFire {
        fn id(&self) -> &RuleId {
            &self.id
        }
        fn severity(&self) -> Severity {
            Severity::MustFix
        }
        fn description(&self) -> &str {
            "test rule that fires on every chunk"
        }
        fn fix_guidance(&self) -> &FixGuidance {
            &self.fix
        }
        fn check(&self, ctx: &LintContext<'_>) -> Vec<Finding> {
            let root = ctx.tree.root_node();
            vec![
                ctx.finding(self, root, "chunk seen", "test rule always fires")
                    .fix("no-op fix")
                    .build(),
            ]
        }
    }

    fn make_context<'a>(
        tree: &'a ParsedTree,
        source: &'a str,
        role: &'a LuaFileRole,
        relative_path: &'a Path,
    ) -> LintContext<'a> {
        LintContext {
            tree,
            source,
            role,
            relative_path,
        }
    }

    #[test]
    fn engine_runs_every_rule() {
        let mut parser = LuaParser::new().unwrap();
        let source = "return 1\n";
        let tree = parser.parse(source).unwrap();
        let role = LuaFileRole::Plugin;
        let rel = PathBuf::from("plugin/x.lua");

        let engine = RuleEngine::new(vec![Box::new(AlwaysFire::new())]);
        let ctx = make_context(&tree, source, &role, &rel);
        let findings = engine.check(&ctx);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule.as_str(), "nvim/health-check");
        assert_eq!(findings[0].severity, Severity::MustFix);
        assert_eq!(findings[0].location.file, PathBuf::from("plugin/x.lua"));
        assert_eq!(findings[0].location.line, 1);
        assert_eq!(findings[0].location.column, 1);
        assert_eq!(findings[0].message, "chunk seen");
        assert_eq!(findings[0].fix.as_deref(), Some("no-op fix"));
    }

    #[test]
    fn engine_returns_empty_when_no_rules() {
        let mut parser = LuaParser::new().unwrap();
        let tree = parser.parse("return 1\n").unwrap();
        let role = LuaFileRole::Plugin;
        let rel = PathBuf::from("plugin/x.lua");
        let engine = RuleEngine::default();
        let ctx = make_context(&tree, "return 1\n", &role, &rel);
        assert!(engine.check(&ctx).is_empty());
    }

    #[test]
    fn has_rule_finds_registered_id() {
        let engine = RuleEngine::new(vec![Box::new(AlwaysFire::new())]);
        let id = RuleId::parse("nvim/health-check").unwrap();
        assert!(engine.has_rule(&id));

        let missing = RuleId::parse("nvim/plug-mapping").unwrap();
        assert!(!engine.has_rule(&missing));
    }

    #[test]
    fn locate_maps_tree_sitter_zero_indexed_to_one_indexed() {
        let mut parser = LuaParser::new().unwrap();
        // Line 2 col 8 (0-indexed 1, 7) — check we bump both to 1-indexed.
        let source = "-- header\nlocal x = 1\n";
        let tree = parser.parse(source).unwrap();
        let role = LuaFileRole::Plugin;
        let rel = PathBuf::from("plugin/x.lua");
        let ctx = make_context(&tree, source, &role, &rel);

        // Descend to `local x = 1`.
        let root = tree.root_node();
        let mut cursor = root.walk();
        let stmts: Vec<_> = root.children(&mut cursor).collect();
        let local_stmt = stmts.iter().find(|n| n.kind() == "variable_declaration");
        let node = *local_stmt.expect("tree-sitter-lua parses the local statement");

        let loc = ctx.locate(node);
        assert_eq!(loc.line, 2);
        assert_eq!(loc.column, 1);
    }

    #[test]
    fn text_extracts_source_slice() {
        let mut parser = LuaParser::new().unwrap();
        let source = "local x = 1\n";
        let tree = parser.parse(source).unwrap();
        let role = LuaFileRole::Plugin;
        let rel = PathBuf::from("plugin/x.lua");
        let ctx = make_context(&tree, source, &role, &rel);
        let span = ByteSpan::new(6, 7);
        assert_eq!(ctx.text(span), "x");
    }

    #[test]
    fn finding_builder_omits_fix_when_none() {
        let mut parser = LuaParser::new().unwrap();
        let tree = parser.parse("return 1").unwrap();
        let role = LuaFileRole::Plugin;
        let rel = PathBuf::from("plugin/x.lua");
        let ctx = make_context(&tree, "return 1", &role, &rel);
        let rule = AlwaysFire::new();
        let finding = ctx.finding(&rule, tree.root_node(), "m", "w").build();
        assert!(finding.fix.is_none());
    }

    // Test rule that fires once from check_repo.
    struct RepoAlwaysFire {
        id: RuleId,
        fix: FixGuidance,
    }

    impl RepoAlwaysFire {
        fn new() -> Self {
            Self {
                id: RuleId::parse("nvim/health-check").unwrap(),
                fix: FixGuidance::Manual {
                    description: "test".to_string(),
                },
            }
        }
    }

    impl LintRule for RepoAlwaysFire {
        fn id(&self) -> &RuleId {
            &self.id
        }
        fn severity(&self) -> Severity {
            Severity::MustFix
        }
        fn description(&self) -> &str {
            "test"
        }
        fn fix_guidance(&self) -> &FixGuidance {
            &self.fix
        }
        fn check_repo(&self, ctx: &RepoContext<'_>) -> Vec<Finding> {
            let target = ctx
                .primary_module()
                .map(|m| PathBuf::from(format!("lua/{m}/init.lua")))
                .unwrap_or_else(|| PathBuf::from("."));
            vec![
                ctx.finding_at_file(self, target, "repo-level fire", "w")
                    .fix("f")
                    .build(),
            ]
        }
    }

    #[test]
    fn engine_runs_check_repo_once() {
        let files = vec![
            LuaFile {
                path: PathBuf::from("/tmp/x/lua/foo/init.lua"),
                relative_path: PathBuf::from("lua/foo/init.lua"),
                role: LuaFileRole::LuaInit {
                    module: "foo".to_string(),
                },
            },
            LuaFile {
                path: PathBuf::from("/tmp/x/plugin/foo.lua"),
                relative_path: PathBuf::from("plugin/foo.lua"),
                role: LuaFileRole::Plugin,
            },
        ];
        let engine = RuleEngine::new(vec![Box::new(RepoAlwaysFire::new())]);
        let root = PathBuf::from("/tmp/x");
        let ctx = RepoContext {
            root: &root,
            files: &files,
        };
        let findings = engine.check_repo(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].location.file, PathBuf::from("lua/foo/init.lua"));
        assert_eq!(findings[0].location.line, 1);
        assert_eq!(findings[0].location.column, 1);
    }

    #[test]
    fn per_file_rule_check_repo_defaults_to_empty() {
        let files: Vec<LuaFile> = Vec::new();
        let engine = RuleEngine::new(vec![Box::new(AlwaysFire::new())]);
        let root = PathBuf::from("/tmp/empty");
        let ctx = RepoContext {
            root: &root,
            files: &files,
        };
        assert!(engine.check_repo(&ctx).is_empty());
    }

    #[test]
    fn repo_context_primary_module_finds_first_lua_init() {
        let files = vec![
            LuaFile {
                path: PathBuf::from("/x/lua/foo/config.lua"),
                relative_path: PathBuf::from("lua/foo/config.lua"),
                role: LuaFileRole::Lua {
                    module: "foo".to_string(),
                },
            },
            LuaFile {
                path: PathBuf::from("/x/lua/foo/init.lua"),
                relative_path: PathBuf::from("lua/foo/init.lua"),
                role: LuaFileRole::LuaInit {
                    module: "foo".to_string(),
                },
            },
        ];
        let root = PathBuf::from("/x");
        let ctx = RepoContext {
            root: &root,
            files: &files,
        };
        assert_eq!(ctx.primary_module(), Some("foo"));
    }

    #[test]
    fn repo_context_primary_module_none_when_no_init() {
        let files = vec![LuaFile {
            path: PathBuf::from("/x/plugin/foo.lua"),
            relative_path: PathBuf::from("plugin/foo.lua"),
            role: LuaFileRole::Plugin,
        }];
        let root = PathBuf::from("/x");
        let ctx = RepoContext {
            root: &root,
            files: &files,
        };
        assert_eq!(ctx.primary_module(), None);
    }
}
