//! Locked rule categories for the v0.1.0 rule-name ontology.
//!
//! Categories are the axis along which rules are enabled/disabled in
//! bulk. This list is intentionally short — see the
//! architecture-repo `TODO.md` "Cross-cutting decisions (locked)"
//! section for the design rationale, and treat any addition as a
//! breaking change that requires bumping the ontology version.

/// Every category recognized by [`RuleId::parse`][parse]. Ordered by
/// planned adoption (`nvim` and `deps` ship in v0.1.0; `docs`, `test`,
/// and `ci` are reserved for v0.2.0+).
///
/// [parse]: crate::domain::rule::RuleId::parse
pub const CATEGORIES: &[&str] = &["nvim", "deps", "docs", "test", "ci"];

/// True when `s` matches one of the [`CATEGORIES`] exactly.
///
/// Categories are case-sensitive on purpose — a rule spelled
/// `NVIM/foo` is a different string than `nvim/foo` and would silently
/// bypass any config keyed on the canonical spelling.
pub fn is_locked(s: &str) -> bool {
    CATEGORIES.contains(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_set_matches_ontology() {
        assert_eq!(CATEGORIES, &["nvim", "deps", "docs", "test", "ci"]);
    }

    #[test]
    fn recognizer_is_case_sensitive() {
        assert!(is_locked("nvim"));
        assert!(!is_locked("Nvim"));
        assert!(!is_locked("NVIM"));
    }

    #[test]
    fn rejects_unknown_category() {
        assert!(!is_locked("lint"));
        assert!(!is_locked("style"));
        assert!(!is_locked(""));
    }
}
