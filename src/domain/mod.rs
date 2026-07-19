//! Domain layer — pure types with no I/O.
//!
//! The domain owns the *shape* of the data (rule IDs, severities,
//! findings) and the *invariants* around it (locked categories,
//! kebab-case names, two-word name ceiling). Anything that talks to
//! the filesystem, a parser, or a formatter lives in
//! [`crate::adapters`].

pub mod categories;
pub mod finding;
pub mod rule;
pub mod rule_engine;
pub mod severity;

pub use finding::{ByteSpan, Finding, Location};
pub use rule::{FixGuidance, Rule, RuleId, RuleIdError};
pub use rule_engine::{
    FileContext, FindingBuilder, LintContext, LintRule, RepoContext, RuleEngine,
};
pub use severity::Severity;
