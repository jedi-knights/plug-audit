//! Inline suppression directives.
//!
//! Syntax:
//!
//! ```lua
//! vim.api.nvim_create_augroup("Foo")  -- plug-audit: disable-line nvim/augroup-clear — group created upstream, appending intentional
//! -- plug-audit: disable-next-line nvim/plug-mapping — deliberate leader binding, README documents the choice
//! vim.keymap.set("n", "<leader>ff", ...)
//! ```
//!
//! Two directives are recognized:
//! - `disable-line <rule>` — suppress the given rule on the same
//!   source line as the comment.
//! - `disable-next-line <rule>` — suppress on the line immediately
//!   after the comment.
//!
//! **Reasons are required.** Per `rules/lint-suppression.md`, a bare
//! or unjustified suppression is worse than the original warning
//! (hides both the defect and the intent). Suppressions without an
//! em-dash-separated reason are silently ignored — the rule fires as
//! if the comment were absent, and the reader can see there is a
//! defect to fix.
//!
//! The type itself is a plain line → rule-ID lookup table; the
//! tree-sitter walk that populates it lives in the CLI (see
//! [`crate::cli::check`]).

use std::collections::{HashMap, HashSet};

use crate::domain::finding::Finding;

#[derive(Debug, Default)]
pub struct Suppressions {
    per_line: HashMap<u32, HashSet<String>>,
}

impl Suppressions {
    /// Insert a suppression at a specific 1-indexed source line.
    pub fn insert(&mut self, line: u32, rule_id: String) {
        self.per_line.entry(line).or_default().insert(rule_id);
    }

    /// True when a rule of the given ID is suppressed at the finding's
    /// line.
    pub fn is_suppressed(&self, finding: &Finding) -> bool {
        self.per_line
            .get(&finding.location.line)
            .is_some_and(|rules| rules.contains(finding.rule.as_str()))
    }

    /// Count of unique (line, rule) suppression entries. Exposed for
    /// test assertions and diagnostics.
    pub fn len(&self) -> usize {
        self.per_line.values().map(HashSet::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.per_line.is_empty()
    }

    /// Parse one comment's text into a `(target_line, rule_id)` pair.
    /// Returns `None` when the comment is not a suppression directive
    /// or when the reason (em-dash-separated) is missing or empty.
    ///
    /// `comment_line` is the 1-indexed source line the comment starts
    /// on; `disable-next-line` returns `comment_line + 1`.
    pub fn parse_directive(comment_text: &str, comment_line: u32) -> Option<(u32, String)> {
        // Strip both single-line (`--`) and long-form (`--[[...]]`)
        // comment markers. For v0.1.0 we only recognize the single-line
        // form; long-form comments as suppression directives are rare
        // and their parsing has extra corner cases (matching `]]`).
        let inner = comment_text.trim();
        let body = inner.strip_prefix("--")?.trim_start();
        if body.starts_with("[[") {
            return None;
        }

        let after_prefix = body.strip_prefix("plug-audit:")?.trim_start();

        let (kind_offset, after_kind) =
            if let Some(rest) = after_prefix.strip_prefix("disable-next-line") {
                (1u32, rest)
            } else if let Some(rest) = after_prefix.strip_prefix("disable-line") {
                (0u32, rest)
            } else {
                return None;
            };

        let after_kind = after_kind.trim_start();
        // Split into rule-id-part and reason on the em-dash separator.
        let (rule_id_part, reason) = after_kind.split_once('—')?;

        let rule_id = rule_id_part.trim();
        let reason = reason.trim();
        if rule_id.is_empty() || reason.is_empty() {
            return None;
        }

        Some((comment_line + kind_offset, rule_id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::finding::{ByteSpan, Location};
    use crate::domain::rule::RuleId;
    use crate::domain::severity::Severity;
    use std::path::PathBuf;

    fn finding_at(rule_id: &str, line: u32) -> Finding {
        Finding {
            rule: RuleId::parse(rule_id).unwrap(),
            severity: Severity::ShouldFix,
            location: Location {
                file: PathBuf::from("plugin/x.lua"),
                line,
                column: 1,
                byte_span: ByteSpan::new(0, 0),
            },
            message: "m".to_string(),
            why: "w".to_string(),
            fix: None,
        }
    }

    #[test]
    fn is_suppressed_matches_line_and_rule() {
        let mut s = Suppressions::default();
        s.insert(5, "nvim/augroup-clear".to_string());
        assert!(s.is_suppressed(&finding_at("nvim/augroup-clear", 5)));
        assert!(!s.is_suppressed(&finding_at("nvim/augroup-clear", 6)));
        assert!(!s.is_suppressed(&finding_at("nvim/plug-mapping", 5)));
    }

    #[test]
    fn parse_disable_line_returns_same_line() {
        let d = Suppressions::parse_directive(
            "-- plug-audit: disable-line nvim/augroup-clear — group is created upstream",
            10,
        );
        assert_eq!(d, Some((10, "nvim/augroup-clear".to_string())));
    }

    #[test]
    fn parse_disable_next_line_returns_next_line() {
        let d = Suppressions::parse_directive(
            "-- plug-audit: disable-next-line nvim/plug-mapping — README documents the default",
            10,
        );
        assert_eq!(d, Some((11, "nvim/plug-mapping".to_string())));
    }

    #[test]
    fn parse_tolerates_extra_leading_whitespace_after_dashes() {
        let d = Suppressions::parse_directive(
            "--    plug-audit:  disable-line  nvim/augroup-clear   —   reason text",
            5,
        );
        assert_eq!(d, Some((5, "nvim/augroup-clear".to_string())));
    }

    #[test]
    fn parse_ignores_missing_reason() {
        // No em-dash separator → no reason → we treat the directive as
        // absent (per rules/lint-suppression.md).
        let d = Suppressions::parse_directive("-- plug-audit: disable-line nvim/augroup-clear", 5);
        assert_eq!(d, None);
    }

    #[test]
    fn parse_ignores_empty_reason() {
        // Em-dash present but no text after → still no reason.
        let d =
            Suppressions::parse_directive("-- plug-audit: disable-line nvim/augroup-clear —   ", 5);
        assert_eq!(d, None);
    }

    #[test]
    fn parse_ignores_non_suppression_comments() {
        assert_eq!(
            Suppressions::parse_directive("-- ordinary comment", 5),
            None
        );
        assert_eq!(Suppressions::parse_directive("-- TODO: fix later", 5), None);
    }

    #[test]
    fn parse_ignores_unknown_directive() {
        // `disable-file` doesn't exist in v0.1.0 — silently skip.
        assert_eq!(
            Suppressions::parse_directive("-- plug-audit: disable-file nvim/foo — reason", 5),
            None
        );
    }

    #[test]
    fn parse_ignores_long_form_comment() {
        // Long-form Lua comments (`--[[ ... ]]`) are deliberately out
        // of scope for v0.1.0.
        assert_eq!(
            Suppressions::parse_directive(
                "--[[ plug-audit: disable-line nvim/augroup-clear — reason ]]",
                5
            ),
            None
        );
    }

    #[test]
    fn parse_returns_none_for_empty_rule_id() {
        assert_eq!(
            Suppressions::parse_directive("-- plug-audit: disable-line  — reason", 5),
            None
        );
    }

    #[test]
    fn insert_and_len_track_unique_entries() {
        let mut s = Suppressions::default();
        s.insert(1, "nvim/a".to_string());
        s.insert(1, "nvim/b".to_string());
        s.insert(1, "nvim/a".to_string()); // duplicate
        s.insert(2, "nvim/a".to_string());
        assert_eq!(s.len(), 3);
        assert!(!s.is_empty());
    }
}
