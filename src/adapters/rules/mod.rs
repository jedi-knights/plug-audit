//! Built-in rule registry.
//!
//! [`built_in_rules`] returns every rule bundled with the plug-audit
//! binary in registration order. The engine ([`RuleEngine`][engine])
//! preserves that order in its findings; the reporter is what re-groups
//! output by severity per `rules/findings-format.md`.
//!
//! [engine]: crate::domain::rule_engine::RuleEngine

pub mod deps;
pub mod nvim;

use crate::domain::LintRule;

/// Every rule the tool ships with. Registration order is stable —
/// downstream tooling that snapshots rule output relies on it.
pub fn built_in_rules() -> Vec<Box<dyn LintRule>> {
    vec![
        Box::new(nvim::AugroupClear),
        Box::new(nvim::HealthCheck),
        Box::new(nvim::PlugMapping),
        Box::new(deps::OptionalPeer),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RuleEngine;

    #[test]
    fn registry_contains_expected_rules() {
        let ids: Vec<String> = built_in_rules()
            .iter()
            .map(|r| r.id().as_str().to_string())
            .collect();
        insta::assert_json_snapshot!("built_in_rules_registry", ids);
    }

    #[test]
    fn registry_composes_with_engine() {
        let engine = RuleEngine::new(built_in_rules());
        assert!(!engine.rules().is_empty());
    }

    #[test]
    fn every_rule_has_a_unique_id() {
        let rules = built_in_rules();
        let mut seen = std::collections::HashSet::new();
        for r in &rules {
            let id = r.id().as_str().to_string();
            assert!(seen.insert(id.clone()), "duplicate rule ID: {id}");
        }
    }
}
