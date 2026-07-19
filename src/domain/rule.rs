//! Rule identifier and metadata.
//!
//! [`RuleId`] is the stable public identifier for a rule (e.g.
//! `nvim/augroup-clear`) and validates the ontology format
//! `<category>/<kebab-case-name>` at construction time. Serde
//! deserialization runs the same validator, so config files with a
//! malformed rule ID fail loud rather than silently ignoring the entry.

use serde::{Deserialize, Serialize};

use crate::domain::categories;
use crate::domain::severity::Severity;

/// Maximum words in the name portion — locked at 2 by the ontology
/// discipline ("three words means the rule is really two rules").
const MAX_NAME_WORDS: usize = 2;

/// Stable rule identifier. Format: `<category>/<kebab-case-name>`.
///
/// Construct via [`RuleId::parse`] or `TryFrom<&str>`; there is no
/// safe way to build an invalid `RuleId` at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId {
    category: &'static str,
    name: String,
    combined: String,
}

impl RuleId {
    /// Parse a rule ID string. Rejects unknown categories, malformed
    /// names, and any name with more than [`MAX_NAME_WORDS`] words.
    pub fn parse(s: &str) -> Result<Self, RuleIdError> {
        let (category, name) = s
            .split_once('/')
            .ok_or_else(|| RuleIdError::MissingSeparator(s.to_string()))?;

        if category.is_empty() {
            return Err(RuleIdError::EmptyCategory(s.to_string()));
        }
        if name.is_empty() {
            return Err(RuleIdError::EmptyName(s.to_string()));
        }

        let category = categories::CATEGORIES
            .iter()
            .find(|c| **c == category)
            .copied()
            .ok_or_else(|| RuleIdError::UnknownCategory {
                category: category.to_string(),
                allowed: categories::CATEGORIES,
            })?;

        validate_name(name)?;

        Ok(Self {
            category,
            name: name.to_string(),
            combined: format!("{}/{}", category, name),
        })
    }

    /// Category portion (e.g. `"nvim"` for `nvim/augroup-clear`).
    pub fn category(&self) -> &'static str {
        self.category
    }

    /// Name portion (e.g. `"augroup-clear"` for `nvim/augroup-clear`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Full identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.combined
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.combined)
    }
}

impl std::str::FromStr for RuleId {
    type Err = RuleIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for RuleId {
    type Error = RuleIdError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl Serialize for RuleId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.combined)
    }
}

impl<'de> Deserialize<'de> for RuleId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

fn validate_name(name: &str) -> Result<(), RuleIdError> {
    // Character-level validation runs before word counting so that
    // `foo--bar`, `-foo`, `foo-`, `Foo`, and `foo_bar` all fail with
    // InvalidNameFormat rather than being miscounted as N words.
    let bad_charset = !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if bad_charset || name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return Err(RuleIdError::InvalidNameFormat(name.to_string()));
    }

    let words: Vec<&str> = name.split('-').collect();

    // Every word (including the first) must start with a lowercase
    // letter. This rejects `lua-5-1` (word "5" is digit-led) even
    // though every character is individually legal.
    for word in &words {
        let first = word.chars().next().unwrap();
        if !first.is_ascii_lowercase() {
            return Err(RuleIdError::InvalidNameFormat(name.to_string()));
        }
    }

    if words.len() > MAX_NAME_WORDS {
        return Err(RuleIdError::TooManyWords {
            name: name.to_string(),
            max: MAX_NAME_WORDS,
        });
    }

    Ok(())
}

/// Structured error for [`RuleId::parse`]. Every variant carries the
/// offending input so config-file diagnostics can point at the exact
/// value that failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleIdError {
    #[error("rule ID `{0}` is missing the `<category>/<name>` separator")]
    MissingSeparator(String),

    #[error("rule ID `{0}` has an empty category portion")]
    EmptyCategory(String),

    #[error("rule ID `{0}` has an empty name portion")]
    EmptyName(String),

    #[error("rule ID category `{category}` is not one of {allowed:?}")]
    UnknownCategory {
        category: String,
        allowed: &'static [&'static str],
    },

    #[error(
        "rule ID name `{0}` must be lowercase kebab-case \
        (start with a-z, contain only a-z, 0-9, and single `-` between words)"
    )]
    InvalidNameFormat(String),

    #[error("rule ID name `{name}` exceeds {max}-word maximum")]
    TooManyWords { name: String, max: usize },
}

/// Guidance describing what to change when a rule fires. `AutoFixable`
/// means the tool can rewrite the source; `Manual` means the fix is
/// context-dependent and only the guidance text is emitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FixGuidance {
    AutoFixable { description: String },
    Manual { description: String },
}

impl FixGuidance {
    pub fn description(&self) -> &str {
        match self {
            Self::AutoFixable { description } | Self::Manual { description } => description,
        }
    }

    pub fn is_auto_fixable(&self) -> bool {
        matches!(self, Self::AutoFixable { .. })
    }
}

/// Static metadata for a rule. The runtime finding (see
/// [`crate::domain::finding::Finding`]) references this by [`RuleId`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub severity: Severity,
    pub description: String,
    pub fix_guidance: FixGuidance,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_locked_categories() {
        for cat in categories::CATEGORIES {
            let id = RuleId::parse(&format!("{}/foo", cat)).unwrap();
            assert_eq!(id.category(), *cat);
            assert_eq!(id.name(), "foo");
        }
    }

    #[test]
    fn parse_accepts_two_word_kebab_names() {
        let id = RuleId::parse("nvim/augroup-clear").unwrap();
        assert_eq!(id.as_str(), "nvim/augroup-clear");
        assert_eq!(id.name(), "augroup-clear");
    }

    #[test]
    fn parse_accepts_digits_in_word_body() {
        // Digits are legal in the body of a word (`lua51`) but not as
        // a word initial (`lua-5-1` splits into three digit-led words).
        let id = RuleId::parse("nvim/lua51").unwrap();
        assert_eq!(id.name(), "lua51");
    }

    #[test]
    fn parse_rejects_digit_led_word() {
        assert!(matches!(
            RuleId::parse("nvim/lua-5"),
            Err(RuleIdError::InvalidNameFormat(_))
        ));
    }

    #[test]
    fn parse_rejects_missing_separator() {
        assert!(matches!(
            RuleId::parse("nvimaugroupclear"),
            Err(RuleIdError::MissingSeparator(_))
        ));
    }

    #[test]
    fn parse_rejects_empty_category() {
        assert!(matches!(
            RuleId::parse("/foo"),
            Err(RuleIdError::EmptyCategory(_))
        ));
    }

    #[test]
    fn parse_rejects_empty_name() {
        assert!(matches!(
            RuleId::parse("nvim/"),
            Err(RuleIdError::EmptyName(_))
        ));
    }

    #[test]
    fn parse_rejects_unknown_category() {
        let err = RuleId::parse("style/foo").unwrap_err();
        assert!(matches!(err, RuleIdError::UnknownCategory { .. }));
    }

    #[test]
    fn parse_rejects_uppercase() {
        assert!(matches!(
            RuleId::parse("nvim/AugroupClear"),
            Err(RuleIdError::InvalidNameFormat(_))
        ));
    }

    #[test]
    fn parse_rejects_leading_digit() {
        assert!(matches!(
            RuleId::parse("nvim/1abc"),
            Err(RuleIdError::InvalidNameFormat(_))
        ));
    }

    #[test]
    fn parse_rejects_leading_or_trailing_hyphen() {
        assert!(matches!(
            RuleId::parse("nvim/-foo"),
            Err(RuleIdError::InvalidNameFormat(_))
        ));
        assert!(matches!(
            RuleId::parse("nvim/foo-"),
            Err(RuleIdError::InvalidNameFormat(_))
        ));
    }

    #[test]
    fn parse_rejects_double_hyphen() {
        assert!(matches!(
            RuleId::parse("nvim/foo--bar"),
            Err(RuleIdError::InvalidNameFormat(_))
        ));
    }

    #[test]
    fn parse_rejects_underscore() {
        assert!(matches!(
            RuleId::parse("nvim/foo_bar"),
            Err(RuleIdError::InvalidNameFormat(_))
        ));
    }

    #[test]
    fn parse_rejects_three_word_names() {
        let err = RuleId::parse("nvim/foo-bar-baz").unwrap_err();
        assert!(matches!(err, RuleIdError::TooManyWords { max: 2, .. }));
    }

    #[test]
    fn serde_roundtrip_ok() {
        let id = RuleId::parse("nvim/plug-mapping").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"nvim/plug-mapping\"");
        let round: RuleId = serde_json::from_str(&json).unwrap();
        assert_eq!(round, id);
    }

    #[test]
    fn serde_rejects_invalid() {
        let err = serde_json::from_str::<RuleId>("\"style/foo\"");
        assert!(err.is_err());
    }

    #[test]
    fn from_str_delegates_to_parse() {
        use std::str::FromStr;
        let id = RuleId::from_str("deps/pcall-optional").unwrap();
        assert_eq!(id.category(), "deps");
    }

    #[test]
    fn fix_guidance_accessors() {
        let auto = FixGuidance::AutoFixable {
            description: "insert clear=true".to_string(),
        };
        assert!(auto.is_auto_fixable());
        assert_eq!(auto.description(), "insert clear=true");

        let manual = FixGuidance::Manual {
            description: "context-dependent — see docs".to_string(),
        };
        assert!(!manual.is_auto_fixable());
    }
}
