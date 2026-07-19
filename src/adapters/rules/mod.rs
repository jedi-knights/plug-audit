//! Built-in rule registry.
//!
//! [`built_in_rules`] returns every rule bundled with the plug-audit
//! binary in registration order. The engine ([`RuleEngine`][engine])
//! preserves that order in its findings; the reporter is what re-groups
//! output by severity per `rules/findings-format.md`.
//!
//! Populated in PA-5 (five rules for v0.1.0). PA-4 ships the empty
//! registry so the wiring is testable and the CLI can list rules from
//! day one.
//!
//! [engine]: crate::domain::rule_engine::RuleEngine

use crate::domain::LintRule;

/// Every rule the tool ships with. Returns an empty `Vec` until PA-5
/// lands the first rule impls.
pub fn built_in_rules() -> Vec<Box<dyn LintRule>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RuleEngine;

    #[test]
    fn scaffold_returns_empty_registry() {
        // Locks the "empty until PA-5" contract — the CLI wiring in PA-6
        // must handle a zero-rule engine cleanly, and this test breaks
        // (loudly, with a review prompt) the moment PA-5 lands.
        assert!(built_in_rules().is_empty());
    }

    #[test]
    fn empty_registry_composes_with_engine() {
        let engine = RuleEngine::new(built_in_rules());
        assert_eq!(engine.rules().len(), 0);
    }
}
