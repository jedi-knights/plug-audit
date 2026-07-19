//! Findings — the runtime output of a rule.
//!
//! Shape matches [`rules/findings-format.md`][fmt]: file:line, what,
//! why, and (for Must Fix + Should Fix) a fix line. The JSON and
//! human-readable representations are locked contracts; snapshot tests
//! at the bottom of this file freeze both.
//!
//! [fmt]: https://github.com/ocrosby/claude-config/blob/main/rules/findings-format.md

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::rule::RuleId;
use crate::domain::severity::Severity;

/// Byte offsets into the source file — half-open, `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        debug_assert!(
            start <= end,
            "byte span start ({start}) must be <= end ({end})"
        );
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Physical location of a finding. Line and column are 1-indexed so
/// `file.lua:42:7` in output matches editor jump conventions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub byte_span: ByteSpan,
}

impl Location {
    pub fn new(file: impl Into<PathBuf>, line: u32, column: u32, byte_span: ByteSpan) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            byte_span,
        }
    }
}

/// A single rule violation. `fix` is required for [`Severity::MustFix`]
/// and [`Severity::ShouldFix`] per the findings-format rule; it is
/// optional for [`Severity::Consider`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub rule: RuleId,
    pub severity: Severity,
    pub location: Location,
    /// One-sentence "what fired".
    pub message: String,
    /// One-sentence "why it matters".
    pub why: String,
    /// One or two sentences on the fix. Required for Must Fix and
    /// Should Fix, optional for Consider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Finding {
    /// True when the finding satisfies the required-fields contract
    /// from `rules/findings-format.md`. Enforced when a rule is
    /// registered; violations at runtime indicate a coding defect,
    /// not user input.
    pub fn has_required_fix(&self) -> bool {
        match self.severity {
            Severity::MustFix | Severity::ShouldFix => self.fix.is_some(),
            Severity::Consider => true,
        }
    }
}

impl std::fmt::Display for Finding {
    /// Canonical one-line console form:
    /// ``` text
    /// `path/to/file.ext:LINE` — <what>. **Why:** <why>. **Fix:** <fix>.
    /// ```
    /// Matches the finding shape in `rules/findings-format.md`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`{}:{}` — {}. **Why:** {}.",
            self.location.file.display(),
            self.location.line,
            self.message.trim_end_matches('.'),
            self.why.trim_end_matches('.'),
        )?;
        if let Some(fix) = &self.fix {
            write!(f, " **Fix:** {}.", fix.trim_end_matches('.'))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_finding(severity: Severity, fix: Option<&str>) -> Finding {
        Finding {
            rule: RuleId::parse("nvim/augroup-clear").unwrap(),
            severity,
            location: Location::new(
                PathBuf::from("plugin/example.lua"),
                42,
                7,
                ByteSpan::new(1024, 1067),
            ),
            message: "augroup created without `clear = true`".to_string(),
            why: "re-sourcing the plugin duplicates autocmds without an explicit clear".to_string(),
            fix: fix.map(str::to_string),
        }
    }

    #[test]
    fn byte_span_len_is_end_minus_start() {
        let span = ByteSpan::new(10, 25);
        assert_eq!(span.len(), 15);
        assert!(!span.is_empty());
    }

    #[test]
    fn byte_span_empty_when_start_equals_end() {
        let span = ByteSpan::new(10, 10);
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
    }

    #[test]
    fn required_fix_present_for_must_fix() {
        let f = sample_finding(
            Severity::MustFix,
            Some("add `{ clear = true }` as second arg"),
        );
        assert!(f.has_required_fix());
    }

    #[test]
    fn required_fix_missing_for_must_fix() {
        let f = sample_finding(Severity::MustFix, None);
        assert!(!f.has_required_fix());
    }

    #[test]
    fn required_fix_optional_for_consider() {
        let f = sample_finding(Severity::Consider, None);
        assert!(f.has_required_fix());
    }

    #[test]
    fn display_matches_findings_format_shape() {
        let f = sample_finding(
            Severity::ShouldFix,
            Some("add `{ clear = true }` as second arg"),
        );
        insta::assert_snapshot!("finding_display_should_fix", f.to_string());
    }

    #[test]
    fn display_omits_fix_when_none() {
        let f = sample_finding(Severity::Consider, None);
        insta::assert_snapshot!("finding_display_consider_no_fix", f.to_string());
    }

    #[test]
    fn json_shape_is_stable() {
        let f = sample_finding(Severity::MustFix, Some("scaffold a stub health.lua"));
        insta::assert_json_snapshot!("finding_json_must_fix", f);
    }

    #[test]
    fn json_omits_fix_field_when_none() {
        let f = sample_finding(Severity::Consider, None);
        insta::assert_json_snapshot!("finding_json_consider_no_fix", f);
    }

    #[test]
    fn json_roundtrip() {
        let original = sample_finding(Severity::MustFix, Some("wrap in pcall"));
        let json = serde_json::to_string(&original).unwrap();
        let round: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(round, original);
    }
}
