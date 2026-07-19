<div align="center">

# plug-audit

**Static analyzer for Neovim plugin repos.**

[![CI](https://github.com/jedi-knights/plug-audit/actions/workflows/ci.yml/badge.svg)](https://github.com/jedi-knights/plug-audit/actions/workflows/ci.yml)
[![Release](https://github.com/jedi-knights/plug-audit/actions/workflows/release.yml/badge.svg)](https://github.com/jedi-knights/plug-audit/actions/workflows/release.yml)
[![Dist](https://github.com/jedi-knights/plug-audit/actions/workflows/dist.yml/badge.svg)](https://github.com/jedi-knights/plug-audit/actions/workflows/dist.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Badge](https://github.com/jedi-knights/plug-audit/actions/workflows/badge.yml/badge.svg)](https://github.com/jedi-knights/plug-audit/actions/workflows/badge.yml)
![Coverage](https://img.shields.io/badge/Coverage-95.3%25-brightgreen)

[Install](#install) · [Quick start](#quick-start) · [Rules](#rules) · [Configuration](#configuration) · [Suppressions](#suppressions) · [CLI reference](#cli-reference) · [Contributing](#contributing)

</div>

---

plug-audit reads a Neovim plugin repository the way a reviewer would — checking for the small, universal conventions well-behaved plugins converge on — and prints the violations as file:line findings you can fix, review, or knowingly suppress with an inline reason.

```
$ plug-audit check .

## Findings

### Must Fix
- `lua/my-plugin/init.lua:1` — plugin repo `my-plugin` is missing `lua/my-plugin/health.lua`. **Why:** users running `:checkhealth` cannot self-diagnose missing deps, wrong Neovim version, or absent peer plugins without a healthcheck module. **Fix:** create `lua/my-plugin/health.lua` with a `check()` function.  [nvim/health-check]

### Should Fix
- `plugin/my-plugin.lua:2` — `require("telescope")` is not guarded by pcall. **Why:** an optional peer must degrade silently — an unguarded require throws and takes the whole plugin down when the peer is absent. **Fix:** rewrite as `local ok, m = pcall(require, "telescope")` and branch on `ok`.  [deps/optional-peer]

2 finding(s) — 1 Must Fix, 1 Should Fix, 0 Consider.
```

## Why plug-audit?

**Codifies the conventions every long-lived Neovim plugin ends up following.** Bare `require("<peer>")` explodes when the peer is missing. `nvim_create_augroup` without `{ clear = true }` duplicates autocmds every time the file is re-sourced. Default `<leader>` keymaps in `plugin/` steal keys from the user. Commands that reach into `require("<name>").config` before `setup()` ran silently nil-index at the worst moment. Every one of these bugs is opaque, silent, and reproducible — and every plugin author solves them the same way after learning the hard way.

plug-audit is that hard-earned checklist as executable rules. Point it at a plugin repo and it reports what would trip a careful reviewer:

- Augroup hygiene, `<Plug>`-first keymaps, setup-optional command surface, `pcall`-guarded optional peers, and a proper `:checkhealth` module — enforced as rules, not folk wisdom
- Findings grouped as **Must Fix** / **Should Fix** / **Consider** so a CI run tells the whole team what to prioritize
- Console output for humans and JSON output for CI on the same command
- Configurable per-rule and per-category via a small TOML file, with inline suppressions that require an explanation

The tool is a single binary. No Lua runtime, no Neovim install, no vendored dependencies — cross-compiles clean on any GitHub-hosted runner.

## Install

### From source

```bash
cargo install --git https://github.com/jedi-knights/plug-audit
```

Pre-built binaries and a Homebrew tap (`jedi-knights/tap/plug-audit`) are planned for the v0.1.0 tag.

## Quick start

Run against the current directory:

```bash
plug-audit check
```

Or a specific repo:

```bash
plug-audit check ~/src/github.com/me/my-plugin.nvim
```

For CI:

```bash
plug-audit check . --strict --format=json
```

`--strict` returns exit code 2 when any **Must Fix** finding is present, which most CI parsers already understand as failure. `--format=json` writes a stable envelope with a `findings` array and a `summary` object; both are documented under [CLI reference](#cli-reference).

## Rules

All five ship enabled at their default severity. Full per-rule documentation is in [`docs/rules/`](docs/rules/).

| Rule | Severity | Auto-fix | What it checks |
|---|---|---|---|
| [`nvim/augroup-clear`](docs/rules/nvim-augroup-clear.md) | Should Fix | manual | `vim.api.nvim_create_augroup` must pass `{ clear = true }` so re-sourcing does not duplicate autocmds |
| [`nvim/health-check`](docs/rules/nvim-health-check.md) | Must Fix | manual | Every plugin repo must ship `lua/<name>/health.lua` for `:checkhealth <name>` |
| [`nvim/plug-mapping`](docs/rules/nvim-plug-mapping.md) | Should Fix | manual | `plugin/*.lua` must not ship default `<leader>` keymaps; expose `<Plug>` mappings and let the user bind their own key |
| [`nvim/setup-optional`](docs/rules/nvim-setup-optional.md) | Must Fix | manual | Commands defined in `plugin/*.lua` must not depend on `setup()` — reaching into `require("<name>").config` is the tell |
| [`deps/optional-peer`](docs/rules/deps-optional-peer.md) | Should Fix | manual | `require("<peer>")` for optional peer plugins must be wrapped in `pcall`; `vim.*` and `plenary.*` are exempt |

Auto-fix support is deferred until at least one rule ships an autofixable form; see [`docs/rules/`](docs/rules/) for what each rule does and does not do.

## Configuration

Drop a `.plug-audit.toml` at the repo root — auto-discovered when `--config` is not passed. Explicit `--config <path>` overrides auto-discovery; a missing explicit config file is a tool error (`exit 1`) so a typoed path fails loud.

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

**Precedence** (highest to lowest):
1. CLI flags
2. Per-rule `[rules]` entry
3. Per-category `[categories]` entry
4. Built-in default (enabled at metadata severity)

Unknown rule IDs and category names fail validation with the offending identifier surfaced via anyhow's error chain — a config typo cannot silently disable nothing.

## Suppressions

Suppress a single finding with a magic comment. **Reasons are required** — a bare or unjustified suppression is worse than the original warning (hides the defect *and* the intent behind hiding it), so directives without an em-dash-separated reason are silently ignored.

```lua
-- Same-line
vim.api.nvim_create_augroup("Foo")  -- plug-audit: disable-line nvim/augroup-clear — group is created upstream, appending intentional

-- Preceding-line
-- plug-audit: disable-next-line nvim/plug-mapping — README documents the deliberate default keybinding
vim.keymap.set("n", "<leader>?", callback)
```

Repo-level rules (`nvim/health-check`) are intentionally not line-suppressible — they would need a file- or repo-level directive; deferred.

## CLI reference

```
plug-audit check [PATH] [OPTIONS]
```

| Flag | Values | Default | Description |
|---|---|---|---|
| `PATH` | directory | `.` | Directory to scan |
| `--format` | `console` \| `json` | `console` | Output format |
| `--config` | file path | *(auto-discover `.plug-audit.toml`)* | Explicit config file. Missing file → exit 1 |
| `--strict` | — | off | Exit 2 when any Must Fix finding is present |

### Exit codes

Locked contract.

| Code | Meaning |
|---|---|
| `0` | Tool ran to completion. Findings, if any, are emitted; the caller decides what to do next |
| `1` | Tool error — discovery failed, parser init failed, or config validation rejected the config |
| `2` | `--strict` was set and at least one Must Fix finding fired |

### JSON output

`--format=json` writes a stable envelope; consumers who need feature detection should check specific fields, not compare `version` strings. `fix` is omitted for Consider findings.

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

## Development

Rust 1.95 pinned via `rust-toolchain.toml`. Standard `rustup` install; `cargo` on `PATH`.

```bash
cargo build
cargo test          # runs unit + integration
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

### Adding a rule

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
- Commits follow Angular Conventional Commits: `<type>(<scope>): <description>`. Types: `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `chore`
- One PR = one `type(scope)` pair. If you can't describe the change in a single subject line, split it
- New rules require the fixture-plus-doc bundle described above

Bug reports and feature requests are welcome as GitHub issues.

## License

MIT — see [LICENSE](LICENSE).
