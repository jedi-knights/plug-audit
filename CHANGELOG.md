# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0](https://github.com/jedi-knights/plug-audit/releases/tag/v0.1.0) - 2026-07-19

### Added

- *(cli)* inline suppression directives that enforce reason text ([#12](https://github.com/jedi-knights/plug-audit/pull/12))
- *(cli)* add TOML config with per-rule/category enable and severity override ([#11](https://github.com/jedi-knights/plug-audit/pull/11))
- *(cli)* add --format=json for machine-readable output ([#10](https://github.com/jedi-knights/plug-audit/pull/10))
- *(cli)* implement plug-audit check with console reporter and exit codes ([#9](https://github.com/jedi-knights/plug-audit/pull/9))
- *(rules)* add nvim/setup-optional — completes v0.1.0 rule set ([#8](https://github.com/jedi-knights/plug-audit/pull/8))
- *(rules)* add nvim/plug-mapping ([#7](https://github.com/jedi-knights/plug-audit/pull/7))
- *(rules)* add nvim/health-check as the first repo-level rule ([#6](https://github.com/jedi-knights/plug-audit/pull/6))
- *(rules)* add deps/optional-peer ([#5](https://github.com/jedi-knights/plug-audit/pull/5))
- *(rules)* add engine repo-check hook and nvim/augroup-clear ([#4](https://github.com/jedi-knights/plug-audit/pull/4))
- *(engine)* add LintRule trait, LintContext, and RuleEngine driver ([#3](https://github.com/jedi-knights/plug-audit/pull/3))
- *(adapters)* add repo discovery and tree-sitter Lua parser ([#2](https://github.com/jedi-knights/plug-audit/pull/2))
- *(domain)* add RuleId, Severity, Finding with locked wire format ([#1](https://github.com/jedi-knights/plug-audit/pull/1))

### Fixed

- *(ci)* release-plz uses GH_TOKEN and triggers on ci completion ([#21](https://github.com/jedi-knights/plug-audit/pull/21))
- *(ci)* badge job uses GH_TOKEN so it can bypass main's PR ruleset ([#19](https://github.com/jedi-knights/plug-audit/pull/19))
- *(ci)* let badge job run on workflow_dispatch as well as push ([#17](https://github.com/jedi-knights/plug-audit/pull/17))

### Other

- *(readme)* restructure to match neospec's shape — drop ToC, add sample output ([#24](https://github.com/jedi-knights/plug-audit/pull/24))
- *(align-with-neospec)* match neospec's workflow names + badge layout ([#23](https://github.com/jedi-knights/plug-audit/pull/23))
- update coverage badge [skip ci]
- split badge into its own workflow triggered by ci completion ([#20](https://github.com/jedi-knights/plug-audit/pull/20))
- add release-plz for automated version bumps and changelog ([#18](https://github.com/jedi-knights/plug-audit/pull/18))
- add workflow_dispatch trigger for manual re-runs ([#16](https://github.com/jedi-knights/plug-audit/pull/16))
- publish live coverage badge via jedi-knights/coverage-badge ([#15](https://github.com/jedi-knights/plug-audit/pull/15))
- *(readme)* full OSS structure with rules, config, suppression, contribution ([#14](https://github.com/jedi-knights/plug-audit/pull/14))
- add cross-platform build + clippy + fmt + test + coverage workflow ([#13](https://github.com/jedi-knights/plug-audit/pull/13))
- *(scaffold)* initialize Rust workspace with hexagonal layout

- Initial v0.1.0-preview development. See the Status section of the
  README for shipped features and remaining v0.1.0 work.
