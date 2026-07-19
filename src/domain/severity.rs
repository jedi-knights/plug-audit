//! Finding severity — the three canonical buckets from
//! [`rules/findings-format.md`][fmt]. No synonyms; a report using
//! "Critical" / "Warning" / "Suggestion" (or similar) is itself a
//! `Should Fix` finding when audited.
//!
//! [fmt]: https://github.com/ocrosby/claude-config/blob/main/rules/findings-format.md

use serde::{Deserialize, Serialize};

/// Canonical severity buckets. Ordered from most to least critical so
/// [`Severity::cmp`] and derived comparisons match report ordering.
///
/// Wire format is lowercase kebab-case: `"must-fix"`, `"should-fix"`,
/// `"consider"`. This is a locked interface — do not add variants,
/// rename, or reorder without updating the JSON snapshot in
/// [`crate::domain::finding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    MustFix,
    ShouldFix,
    Consider,
}

impl Severity {
    /// Human-readable heading for the report bucket, matching the shape
    /// in `rules/findings-format.md` (e.g. `### Must Fix`).
    pub fn heading(self) -> &'static str {
        match self {
            Self::MustFix => "Must Fix",
            Self::ShouldFix => "Should Fix",
            Self::Consider => "Consider",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.heading())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_is_kebab_case() {
        assert_eq!(
            serde_json::to_string(&Severity::MustFix).unwrap(),
            "\"must-fix\""
        );
        assert_eq!(
            serde_json::to_string(&Severity::ShouldFix).unwrap(),
            "\"should-fix\""
        );
        assert_eq!(
            serde_json::to_string(&Severity::Consider).unwrap(),
            "\"consider\""
        );
    }

    #[test]
    fn wire_format_roundtrips() {
        for sev in [Severity::MustFix, Severity::ShouldFix, Severity::Consider] {
            let json = serde_json::to_string(&sev).unwrap();
            let round: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(round, sev);
        }
    }

    #[test]
    fn ord_reflects_criticality() {
        assert!(Severity::MustFix < Severity::ShouldFix);
        assert!(Severity::ShouldFix < Severity::Consider);
    }

    #[test]
    fn heading_matches_findings_format_rule() {
        assert_eq!(Severity::MustFix.heading(), "Must Fix");
        assert_eq!(Severity::ShouldFix.heading(), "Should Fix");
        assert_eq!(Severity::Consider.heading(), "Consider");
    }
}
