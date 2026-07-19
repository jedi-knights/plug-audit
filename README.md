# plug-audit

Static analyzer for Neovim plugin repos.

> **In development.** v0.1.0 target: five rules covering augroup hygiene, `<Plug>` indirection, health-check presence, setup-optional invariants, and optional-peer `pcall` wrapping. See `docs/rules/` once populated.

## Status

Repo scaffolded 2026-07-18. Public API and rule IDs are not yet stable — do not depend on them.

## Positioning

- **Static / compile-time counterpart** to `keymap-forensics.nvim` (runtime `:WhyKey <lhs>` diagnosis)
- Rule format follows [`ocrosby/claude-config` findings-format rule](https://github.com/ocrosby/claude-config/blob/main/rules/findings-format.md): Must Fix / Should Fix / Consider
- Rule IDs follow the `<category>/<kebab-case>` ontology; categories are locked (`nvim`, `deps`, `docs`, `test`, `ci`)

## License

MIT
