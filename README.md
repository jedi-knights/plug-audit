# plug-audit

[![ci](https://github.com/jedi-knights/plug-audit/actions/workflows/ci.yml/badge.svg)](https://github.com/jedi-knights/plug-audit/actions/workflows/ci.yml)
![Coverage](https://img.shields.io/badge/Coverage-0%25-lightgrey)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![rust: 1.95](https://img.shields.io/badge/rust-1.95-orange.svg)](rust-toolchain.toml)

Static analyzer for Neovim plugin repos. Reports what a well-behaved plugin needs — augroup hygiene, `<Plug>`-first keymaps, setup-optional command surface, `pcall`-guarded optional peers, and a proper `:checkhealth` module — as file:line findings you can fix or grep-suppress.

> **Preview release.** The v0.1.0 rule set is complete and the CLI is usable end-to-end. Public API and rule IDs are frozen for v0.1.0 but may evolve after v0.2.0. See [Status](#status) for details.

---

## Table of contents

- [Why plug-audit](#why-plug-audit)
- [Features](#features)
- [Install](#install)
- [Quick start](#quick-start)
- [Rules](#rules)
- [Configuration](#configuration)
- [Inline suppressions](#inline-suppressions)
- [Exit codes](#exit-codes)
- [JSON output](#json-output)
- [Positioning in the jedi-knights portfolio](#positioning-in-the-jedi-knights-portfolio)
- [Status](#status)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

---

## Why plug-audit

Neovim plugin authors converge on a small set of load-bearing conventions — tpope's `<Plug>` idiom, tj's universal `health.lua`, prime's setup-optional invariants, the augroup-clear autocmd-hygiene rule that every long-lived plugin follows. Missing one of these is not a bug the user files against you; it is a paper cut that surfaces later as a duplicated autocmd, a nil-index on `:MyPluginCmd`, or a `:checkhealth` that says "no healthcheck for `myplugin`."

plug-audit codifies those conventions as rules. Point it at a plugin repo and it reports the violations as Must Fix / Should Fix / Consider findings you can review, fix, or knowingly suppress with an inline reason.

## Features

- **Five v0.1.0 rules** covering augroup hygiene, `<Plug>` indirection, health-check presence, setup-optional invariants, and optional-peer `pcall` wrapping
- **Repo-level and per-file rules** in the same engine — health-check fires once on repo shape; augroup-clear fires once per offending call site
- **Console and JSON reporters** — humans read grouped-by-severity markdown; CI reads a stable JSON envelope with a `findings` array and a `summary` object
- **TOML config** — enable/disable individual rules or whole categories, override per-rule severity for `--strict` gating
- **Inline suppressions** — `-- plug-audit: disable-line <rule> — reason` and `-- plug-audit: disable-next-line <rule> — reason`, with the reason enforced
- **`--strict` mode** for CI — exits `2` when any Must Fix finding is present, `0` otherwise

## Install

### From source

```bash
cargo install --git https://github.com/jedi-knights/plug-audit
```

Pre-built binaries and a Homebrew tap are planned; see [Status](#status).

## Quick start

Run against the current directory:

```bash
plug-audit check
```

Or a specific repo:

```bash
plug-audit check ~/src/github.com/me/my-plugin.nvim
```

Sample output on a repo that's missing a health module and has an unguarded optional-peer require:

```
## Findings

### Must Fix

- `lua/my-plugin/init.lua:1` — plugin repo `my-plugin` is missing `lua/my-plugin/health.lua`. **Why:** users running `:checkhealth` cannot self-diagnose missing deps, wrong Neovim version, or absent peer plugins without a healthcheck module. **Fix:** create `lua/my-plugin/health.lua` with a `check()` function reporting external-dep availability, peer-plugin presence, and version constraints.  [nvim/health-check]

### Should Fix

- `plugin/my-plugin.lua:2` — `require("telescope")` is not guarded by pcall. **Why:** an optional peer must degrade silently — an unguarded require throws and takes the whole plugin down when the peer is absent. **Fix:** rewrite as `local ok, m = pcall(require, "telescope")` and branch on `ok`.  [deps/optional-peer]

2 finding(s) — 1 Must Fix, 1 Should Fix, 0 Consider.
```

## Rules

All five ship enabled at their default severity. Full per-rule docs are in [`docs/rules/`](docs/rules/).

| Rule | Severity | Auto-fix | What it checks |
|---|---|---|---|
| [`nvim/augroup-clear`](docs/rules/nvim-augroup-clear.md) | Should Fix | manual | `vim.api.nvim_create_augroup` calls must pass `{ clear = true }` so re-sourcing does not duplicate autocmds |
| [`nvim/health-check`](docs/rules/nvim-health-check.md) | Must Fix | manual | Every plugin repo must ship `lua/<name>/health.lua` for `:checkhealth <name>` |
| [`nvim/plug-mapping`](docs/rules/nvim-plug-mapping.md) | Should Fix | manual | `plugin/*.lua` must not ship default `<leader>` keymaps; expose `<Plug>` mappings and let the user bind their own key |
| [`nvim/setup-optional`](docs/rules/nvim-setup-optional.md) | Must Fix | manual | Commands defined in `plugin/*.lua` must not depend on `setup()` — reaching into `require("<name>").config` is the tell |
| [`deps/optional-peer`](docs/rules/deps-optional-peer.md) | Should Fix | manual | `require("<peer>")` for optional peer plugins must be wrapped in `pcall`; `vim.*` and `plenary.*` are exempt |

Auto-fix support is deferred until at least one rule ships an autofixable form; see [Status](#status).

## Configuration

Drop a `.plug-audit.toml` at the repo root:

```toml
# Disable a single rule
[rules]
"nvim/augroup-clear" = false

# Disable an entire category
[categories]
deps = false

# Override per-rule severity — Should Fix rules can be promoted for CI --strict gating
[severity]
"nvim/plug-mapping" = "must-fix"
```

Precedence (highest to lowest):
1. CLI flags
2. Per-rule `[rules]` entry
3. Per-category `[categories]` entry
4. Built-in default (enabled at metadata severity)

Explicit `--config <path>` overrides auto-discovery. A missing explicit `--config` file is a **tool error** (`exit 1`) so a typoed path fails loud; a missing auto-discovered file is silent. Unknown rule IDs and category names fail validation with the offending identifier surfaced via anyhow's error chain.

## Inline suppressions

Suppress a single finding with a magic comment. **Reasons are required** — a bare or unjustified suppression is worse than the original warning (hides both the defect and the intent), so directives without an em-dash-separated reason are silently ignored.

```lua
-- Same-line
vim.api.nvim_create_augroup("Foo")  -- plug-audit: disable-line nvim/augroup-clear — group is created upstream, appending intentional

-- Preceding-line
-- plug-audit: disable-next-line nvim/plug-mapping — README documents the deliberate default keybinding
vim.keymap.set("n", "<leader>?", callback)
```

Repo-level rules (`nvim/health-check`) are intentionally not line-suppressible — they'd need a file- or repo-level directive; deferred.

## Exit codes

Locked contract.

| Code | Meaning |
|---|---|
| `0` | The tool ran to completion. Findings, if any, are emitted; the caller decides what to do next |
| `1` | Tool error — discovery failed, parser init failed, or config validation rejected the config |
| `2` | `--strict` was set and at least one Must Fix finding fired |

## JSON output

For CI and adjacent tooling:

```bash
plug-audit check . --format=json
```

Wire shape (locked; snapshot-tested):

```json
{
  "version": "0.1.0",
  "findings": [
    {
      "rule": "nvim/health-check",
      "severity": "must-fix",
      "location": { "file": "lua/foo/init.lua", "line": 1, "column": 1, "byte_span": { "start": 0, "end": 0 } },
      "message": "plugin repo `foo` is missing `lua/foo/health.lua`",
      "why": "users running `:checkhealth` cannot self-diagnose missing deps, wrong Neovim version, or absent peer plugins without a healthcheck module",
      "fix": "create `lua/foo/health.lua` with a `check()` function reporting external-dep availability, peer-plugin presence, and version constraints"
    }
  ],
  "summary": { "total": 1, "must_fix": 1, "should_fix": 0, "consider": 0 }
}
```

Consumers who need feature detection should check the presence of specific fields, not compare `version` strings. `fix` is omitted for Consider findings.

## Positioning in the jedi-knights portfolio

plug-audit is one of two static/runtime companions:

- **plug-audit** (this repo) — **static / compile-time**. Scans a plugin repo and reports rule violations.
- **`keymap-forensics.nvim`** (planned) — **runtime**. `:WhyKey <lhs>` diagnoses which plugin bound a key and how.

The two share a mental model — plug-audit prevents the class of bugs `keymap-forensics.nvim` diagnoses — and cross-link in each other's READMEs.

The finding format follows [`ocrosby/claude-config` findings-format rule](https://github.com/ocrosby/claude-config/blob/main/rules/findings-format.md): Must Fix / Should Fix / Consider. Rule IDs follow the `<category>/<kebab-case>` ontology; the locked category set is `nvim`, `deps`, `docs`, `test`, `ci`.

## Status

**Version:** `0.1.0` (preview — unreleased).

Shipped:
- Full v0.1.0 rule set (5 rules)
- `plug-audit check` CLI with console + JSON reporters
- TOML config with rule/category/severity control
- Inline suppression syntax with enforced reasons
- 132+ unit + integration tests; CI on ubuntu-latest with lint + test + coverage jobs

Planned before v0.1.0 tag:
- Coverage 90% gate (measurement is live; gate is deferred until baseline)
- GitHub release automation via `cargo-dist`
- Homebrew tap `jedi-knights/tap/plug-audit`
- Composite GitHub Action `jedi-knights/plug-audit@v0` — downloads the right binary, runs check, exports `findings-count` and `passed`
- Adoption round-1: run against every jedi-knights plugin

Deferred to v0.2.0+:
- `--fix` mode (no auto-fixable rules exist yet)
- Additional rules in `docs/`, `test/`, `ci/` categories
- Alias-tracking for e.g. `local nca = vim.api.nvim_create_augroup`

## Development

### Prerequisites

- Rust 1.95 (pinned via `rust-toolchain.toml`)
- Standard `rustup` install; `cargo` on `PATH`

### Build and test

```bash
cargo build
cargo test          # runs unit + integration
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

### Add or modify a rule

1. Implement under `src/adapters/rules/<category>/<rule_name>.rs`
2. Add a positive fixture (should fire) at `tests/fixtures/rules/<rule-id>/positive.lua`
3. Add a negative fixture (must not fire) at `tests/fixtures/rules/<rule-id>/negative.lua`
4. Add a doc entry at `docs/rules/<rule-id>.md` covering what / why / fix / scope / suppression
5. Register the rule in `src/adapters/rules/mod.rs` — the built-in-rules snapshot will trip, requiring you to accept the update

### Repo layout

```
src/
  domain/          # pure types — Rule, Severity, Finding, Config, Suppressions
  adapters/
    parser/        # tree-sitter-lua wrapper
    repo/          # gitignore-respecting file discovery + role classification
    rules/         # the shipped rule set
  cli/             # clap-based CLI, console + JSON reporters
docs/
  rules/           # per-rule documentation
tests/
  fixtures/        # sample Lua repos for unit and integration tests
  cli.rs           # end-to-end CLI integration tests
```

Hexagonal split — the domain has no I/O; adapters own tree-sitter, ignore-crate walks, and file reads.

## Contributing

Contributions welcome. Before opening a PR:

- Run the full local check: `cargo build && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test`
- Commits follow Angular Conventional Commits: `<type>(<scope>): <description>`. Types: `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `chore`.
- One PR = one `type(scope)` pair. If you can't describe the change in a single subject line, split it.
- New rules require the fixture-plus-doc bundle described above.

Bug reports and feature requests are welcome as GitHub issues.

## License

MIT — see [LICENSE](LICENSE).
