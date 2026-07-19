//! `plug-audit check [path]` — walk a repo, run every registered rule,
//! and emit findings.
//!
//! Wiring:
//! 1. Discover `.lua` files under `path` via the `ignore` crate.
//! 2. Build one [`LintContext`] per file, run every per-file rule.
//! 3. Build one [`RepoContext`] over the discovery output, run every
//!    repo-level rule.
//! 4. Bucket-sort findings by severity and emit via the console
//!    reporter ([`super::report::write_console`]).
//! 5. Exit `0` on success, `1` on tool error, or `2` when
//!    `--strict` is set and any Must Fix finding fired.
//!
//! Failure isolation: a single file that fails to open or parse does
//! not abort the walk. The error is emitted to stderr and the walk
//! continues — we would rather report N-1 rules on a broken file than
//! zero findings across a hundred files.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, ValueEnum};

use crate::adapters::parser::LuaParser;
use crate::adapters::repo::{LuaFile, discover};
use crate::adapters::rules::built_in_rules;
use crate::domain::rule_engine::{LintContext, RepoContext, RuleEngine};
use crate::domain::{Config, Finding, LintRule, Severity};

use super::{report, report_json};

/// Output format for the report. Console is meant for humans; JSON is
/// the wire shape CI and adjacent tooling read.
#[derive(Copy, Clone, Debug, ValueEnum, Default)]
#[value(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Console,
    Json,
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Directory to scan. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format. `console` is grouped by severity for humans;
    /// `json` is a stable wire shape with a `findings` array and a
    /// `summary` object for machine consumers.
    #[arg(long, value_enum, default_value_t = Format::Console)]
    pub format: Format,

    /// Path to a TOML config file. Missing: auto-discover
    /// `<path>/.plug-audit.toml`, or run with defaults. Explicit path
    /// that does not exist is a tool error (exit 1) so config typos
    /// fail loud.
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Exit with code 2 if any Must Fix finding is present. Reserve
    /// for CI — during local iteration, plain exit 0 lets you rerun
    /// without a shell trap.
    #[arg(long)]
    pub strict: bool,
}

pub fn run(args: &CheckArgs) -> ExitCode {
    let all_rules = built_in_rules();
    let known_ids: Vec<&str> = all_rules.iter().map(|r| r.id().as_str()).collect();

    let config = match load_config(args, &known_ids) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("plug-audit: {err}");
            for cause in err.chain().skip(1) {
                eprintln!("  caused by: {cause}");
            }
            return ExitCode::from(1);
        }
    };

    let filtered_rules: Vec<Box<dyn LintRule>> = all_rules
        .into_iter()
        .filter(|r| config.is_rule_enabled(r.id()))
        .collect();

    let files = match discover(&args.path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("plug-audit: discovery failed: {err}");
            return ExitCode::from(1);
        }
    };

    let engine = RuleEngine::new(filtered_rules);

    let repo_ctx = RepoContext {
        root: &args.path,
        files: &files,
    };
    let primary_module = repo_ctx.primary_module().map(str::to_string);

    let mut findings = Vec::new();

    let mut parser = match LuaParser::new() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("plug-audit: parser initialization failed: {err}");
            return ExitCode::from(1);
        }
    };

    for file in &files {
        match check_one_file(&mut parser, &engine, file, primary_module.as_deref()) {
            Ok(mut per_file) => findings.append(&mut per_file),
            Err(err) => {
                eprintln!(
                    "plug-audit: skipping `{}`: {err}",
                    file.relative_path.display()
                );
            }
        }
    }

    findings.extend(engine.check_repo(&repo_ctx));

    config.apply_severity_overrides(&mut findings);

    findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.location.file.cmp(&b.location.file))
            .then_with(|| a.location.line.cmp(&b.location.line))
    });

    let write_result = match args.format {
        Format::Console => report::write_console(&mut std::io::stdout(), &findings),
        Format::Json => report_json::write_json(&mut std::io::stdout(), &findings),
    };
    if let Err(err) = write_result {
        eprintln!("plug-audit: report write failed: {err}");
        return ExitCode::from(1);
    }

    if args.strict && findings.iter().any(|f| f.severity == Severity::MustFix) {
        ExitCode::from(2)
    } else {
        ExitCode::from(0)
    }
}

fn check_one_file(
    parser: &mut LuaParser,
    engine: &RuleEngine,
    file: &LuaFile,
    primary_module: Option<&str>,
) -> anyhow::Result<Vec<Finding>> {
    let source = std::fs::read_to_string(&file.path)?;
    let tree = parser.parse(&source)?;
    let ctx = LintContext {
        tree: &tree,
        source: &source,
        role: &file.role,
        relative_path: &file.relative_path,
        primary_module,
    };
    Ok(engine.check(&ctx))
}

/// Load and validate the config for this run.
///
/// - Explicit `--config <path>`: must exist. Missing file is a tool error.
/// - No flag: auto-discover `<scan-path>/.plug-audit.toml`. Missing is
///   silent — the tool runs with defaults.
///
/// Validation runs against `known_rule_ids` so a typo in a rule or
/// category name fails fast with an actionable message.
fn load_config(args: &CheckArgs, known_rule_ids: &[&str]) -> anyhow::Result<Config> {
    use anyhow::Context;

    let (source, origin): (Option<String>, Option<PathBuf>) = match &args.config {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read --config file `{}`", path.display()))?;
            (Some(raw), Some(path.clone()))
        }
        None => {
            let auto = args.path.join(".plug-audit.toml");
            if auto.exists() {
                let raw = std::fs::read_to_string(&auto).with_context(|| {
                    format!("failed to read auto-discovered config `{}`", auto.display())
                })?;
                (Some(raw), Some(auto))
            } else {
                (None, None)
            }
        }
    };

    let config = match source {
        Some(text) => Config::from_toml(&text).with_context(|| {
            format!(
                "failed to parse config `{}`",
                origin.as_ref().unwrap().display()
            )
        })?,
        None => Config::default(),
    };

    config.validate(known_rule_ids).with_context(|| {
        origin
            .as_ref()
            .map(|p| format!("config validation failed for `{}`", p.display()))
            .unwrap_or_else(|| "config validation failed".to_string())
    })?;

    Ok(config)
}
