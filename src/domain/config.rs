//! Config — the user-facing wire type for enabling/disabling rules,
//! toggling categories, and overriding per-rule severities.
//!
//! Precedence (highest to lowest):
//! 1. CLI flags
//! 2. Per-rule setting in the config (`[rules]` table)
//! 3. Per-category setting in the config (`[categories]` table)
//! 4. Built-in default (every rule enabled at its metadata severity)
//!
//! TOML shape:
//!
//! ```toml
//! # Per-rule enable/disable. Missing rule = enabled.
//! [rules]
//! "nvim/augroup-clear" = false
//!
//! # Bulk category toggle. A rule's own [rules] entry always wins.
//! [categories]
//! deps = false
//!
//! # Per-rule severity override.
//! [severity]
//! "nvim/plug-mapping" = "must-fix"
//! ```
//!
//! Validation is strict: an unknown rule ID or category in any of the
//! three tables produces a tool-error at CLI startup, so a config
//! typo fails loud instead of silently disabling nothing.

use std::collections::HashMap;

use serde::Deserialize;

use crate::domain::categories;
use crate::domain::finding::Finding;
use crate::domain::rule::RuleId;
use crate::domain::severity::Severity;

/// Parsed config file. All fields default to empty; the tool runs
/// with every rule enabled at its metadata severity when nothing is
/// configured.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub rules: HashMap<String, bool>,
    pub categories: HashMap<String, bool>,
    pub severity: HashMap<String, Severity>,
}

impl Config {
    /// Parse a config from a TOML source string.
    pub fn from_toml(source: &str) -> Result<Self, ConfigError> {
        toml::from_str(source).map_err(ConfigError::Parse)
    }

    /// True when the rule with the given ID should run. Checks the
    /// per-rule table first, then the category, then defaults to
    /// enabled.
    pub fn is_rule_enabled(&self, id: &RuleId) -> bool {
        if let Some(&enabled) = self.rules.get(id.as_str()) {
            return enabled;
        }
        if let Some(&enabled) = self.categories.get(id.category()) {
            return enabled;
        }
        true
    }

    /// Apply per-rule severity overrides in-place. Findings whose rule
    /// has no `[severity]` entry are left as-is.
    pub fn apply_severity_overrides(&self, findings: &mut [Finding]) {
        if self.severity.is_empty() {
            return;
        }
        for finding in findings.iter_mut() {
            if let Some(&override_severity) = self.severity.get(finding.rule.as_str()) {
                finding.severity = override_severity;
            }
        }
    }

    /// Validate every rule ID and category name referenced by the
    /// config against the set of known rule IDs. Returns a sorted
    /// list of unknown entries so error messages are stable across
    /// runs.
    pub fn validate(&self, known_rule_ids: &[&str]) -> Result<(), ConfigError> {
        let mut unknown_rules: Vec<String> = Vec::new();

        for id in self.rules.keys().chain(self.severity.keys()) {
            if !known_rule_ids.contains(&id.as_str()) {
                unknown_rules.push(id.clone());
            }
        }

        let mut unknown_categories: Vec<String> = Vec::new();
        for category in self.categories.keys() {
            if !categories::is_locked(category) {
                unknown_categories.push(category.clone());
            }
        }

        unknown_rules.sort();
        unknown_rules.dedup();
        unknown_categories.sort();
        unknown_categories.dedup();

        if unknown_rules.is_empty() && unknown_categories.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Unknown {
                rules: unknown_rules,
                categories: unknown_categories,
            })
        }
    }
}

/// Structured error from config load or validate. Callers format for
/// the CLI seam.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to parse config TOML: {0}")]
    Parse(#[source] toml::de::Error),

    #[error(
        "config references unknown identifier(s) — rules: {rules:?}, categories: {categories:?}"
    )]
    Unknown {
        rules: Vec<String>,
        categories: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::finding::{ByteSpan, Location};
    use std::path::PathBuf;

    fn known_ids() -> Vec<&'static str> {
        vec![
            "nvim/augroup-clear",
            "nvim/health-check",
            "nvim/plug-mapping",
            "nvim/setup-optional",
            "deps/optional-peer",
        ]
    }

    fn finding(rule_id: &str, severity: Severity) -> Finding {
        Finding {
            rule: RuleId::parse(rule_id).unwrap(),
            severity,
            location: Location {
                file: PathBuf::from("x.lua"),
                line: 1,
                column: 1,
                byte_span: ByteSpan::new(0, 0),
            },
            message: "m".to_string(),
            why: "w".to_string(),
            fix: None,
        }
    }

    #[test]
    fn default_config_leaves_all_rules_enabled() {
        let cfg = Config::default();
        for id in known_ids() {
            let rid = RuleId::parse(id).unwrap();
            assert!(cfg.is_rule_enabled(&rid), "default should enable {id}");
        }
    }

    #[test]
    fn per_rule_disable_wins() {
        let toml = r#"
[rules]
"nvim/augroup-clear" = false
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        let disabled = RuleId::parse("nvim/augroup-clear").unwrap();
        let untouched = RuleId::parse("nvim/health-check").unwrap();
        assert!(!cfg.is_rule_enabled(&disabled));
        assert!(cfg.is_rule_enabled(&untouched));
    }

    #[test]
    fn category_disable_applies_to_all_rules_in_category() {
        let toml = r#"
[categories]
deps = false
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        let deps_rule = RuleId::parse("deps/optional-peer").unwrap();
        let nvim_rule = RuleId::parse("nvim/augroup-clear").unwrap();
        assert!(!cfg.is_rule_enabled(&deps_rule));
        assert!(cfg.is_rule_enabled(&nvim_rule));
    }

    #[test]
    fn per_rule_setting_overrides_category() {
        let toml = r#"
[rules]
"deps/optional-peer" = true

[categories]
deps = false
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        let deps_rule = RuleId::parse("deps/optional-peer").unwrap();
        assert!(
            cfg.is_rule_enabled(&deps_rule),
            "per-rule true should beat category-wide false"
        );
    }

    #[test]
    fn severity_override_mutates_findings() {
        let toml = r#"
[severity]
"nvim/plug-mapping" = "must-fix"
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        let mut findings = vec![
            finding("nvim/plug-mapping", Severity::ShouldFix),
            finding("nvim/augroup-clear", Severity::ShouldFix),
        ];
        cfg.apply_severity_overrides(&mut findings);
        assert_eq!(findings[0].severity, Severity::MustFix);
        assert_eq!(findings[1].severity, Severity::ShouldFix);
    }

    #[test]
    fn severity_override_accepts_all_three_buckets() {
        for wire in ["must-fix", "should-fix", "consider"] {
            let toml = format!("[severity]\n\"nvim/plug-mapping\" = \"{wire}\"");
            let cfg = Config::from_toml(&toml).unwrap();
            assert!(cfg.severity.contains_key("nvim/plug-mapping"));
        }
    }

    #[test]
    fn validate_flags_unknown_rule_id() {
        let toml = r#"
[rules]
"nvim/does-not-exist" = false
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        match cfg.validate(&known_ids()) {
            Err(ConfigError::Unknown { rules, .. }) => {
                assert_eq!(rules, vec!["nvim/does-not-exist"]);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn validate_flags_unknown_category() {
        let toml = r#"
[categories]
made-up = false
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        match cfg.validate(&known_ids()) {
            Err(ConfigError::Unknown { categories, .. }) => {
                assert_eq!(categories, vec!["made-up"]);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn validate_flags_unknown_severity_target() {
        let toml = r#"
[severity]
"nvim/does-not-exist" = "must-fix"
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        match cfg.validate(&known_ids()) {
            Err(ConfigError::Unknown { rules, .. }) => {
                assert_eq!(rules, vec!["nvim/does-not-exist"]);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn validate_dedups_id_referenced_by_both_tables() {
        let toml = r#"
[rules]
"nvim/does-not-exist" = false

[severity]
"nvim/does-not-exist" = "must-fix"
        "#;
        let cfg = Config::from_toml(toml).unwrap();
        match cfg.validate(&known_ids()) {
            Err(ConfigError::Unknown { rules, .. }) => {
                assert_eq!(rules, vec!["nvim/does-not-exist"]);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn empty_toml_parses_to_default() {
        let cfg = Config::from_toml("").unwrap();
        assert!(cfg.rules.is_empty());
        assert!(cfg.categories.is_empty());
        assert!(cfg.severity.is_empty());
    }

    #[test]
    fn deny_unknown_top_level_field() {
        // Guard against typo'd table names (e.g. `[severities]`).
        let toml = "[severities]\n";
        assert!(Config::from_toml(toml).is_err());
    }

    #[test]
    fn parse_error_surfaces_as_config_error() {
        let toml = "not = valid = toml";
        let err = Config::from_toml(toml).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }
}
