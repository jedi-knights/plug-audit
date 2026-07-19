# `nvim/health-check`

**Severity**: Must Fix
**Auto-fix**: no (scaffold via `plug-scaffold` — sibling portfolio tool)
**Category**: `nvim/`
**Scope**: repo-level (fires from `LintRule::check_repo`)

## What fires this rule

A plugin repo — any directory containing either `plugin/**/*.lua` or `lua/<name>/init.lua` — where the primary module has no matching `lua/<name>/health.lua`.

Fires on:

```
myplugin/
  plugin/myplugin.lua
  lua/myplugin/init.lua
  -- no lua/myplugin/health.lua
```

Does not fire on:

```
myplugin/
  plugin/myplugin.lua
  lua/myplugin/init.lua
  lua/myplugin/health.lua       -- present, matches primary module
```

Does not fire on a non-plugin repo (no `plugin/` and no `lua/<name>/init.lua`) — those aren't the target audience.

## Why it matters

`:checkhealth <name>` is the first place a user opens when a plugin misbehaves. Without a health module Neovim reports "no healthcheck for `<name>`" and the user cannot self-diagnose. That means:

- Missing external CLI deps (`rg`, `fd`, `gh`, language servers) surface as opaque runtime errors instead of "install this and re-check."
- Optional peer plugins (telescope, snacks, plenary) that the plugin integrates with silently fail when missing.
- Version mismatches (Neovim 0.10 features called on 0.9) explode at runtime instead of at healthcheck.

A healthcheck is documentation-that-runs. Every well-maintained plugin in the ecosystem ships one; the jedi-knights audit that motivated this tool found the gap in all six local plugins.

## Fix

Create `lua/<name>/health.lua`:

```lua
local M = {}

function M.check()
    vim.health.start("<name>")

    -- External dependencies
    if vim.fn.executable("rg") == 1 then
        vim.health.ok("`rg` is installed")
    else
        vim.health.warn("`rg` not found — search fallback is slower", { "install ripgrep" })
    end

    -- Optional peer plugins
    local ok, _ = pcall(require, "plenary")
    if ok then
        vim.health.ok("plenary.nvim is available")
    else
        vim.health.info("plenary.nvim not present — some features disabled")
    end

    -- Version constraints
    if vim.fn.has("nvim-0.10") == 1 then
        vim.health.ok("Neovim >= 0.10")
    else
        vim.health.error("requires Neovim 0.10 or later")
    end
end

return M
```

The `plug-scaffold` companion tool (portfolio TODO PS-N) will generate this stub automatically for new plugins.

## Suppression

Discouraged — a plugin that ships without healthcheck is a plugin that users cannot self-diagnose. If the tool's users genuinely will never need to check dependencies (e.g. a pure-Lua algorithms library published as a plugin), suppress at the repo level via `.plug-audit.toml`:

```toml
[rules]
"nvim/health-check" = false
```

Explain in the same file with a `# reason:` comment.

## Detection notes

- The primary module is the first `lua/<X>/init.lua` (or `lua/<X>.lua`) found in sorted order. Multi-module repos use the first alphabetically.
- The finding anchors at `lua/<name>/init.lua` (the file the reader will open next) rather than the missing file.
- A `plugin/`-only repo with no `lua/<name>/init.lua` is unusual and the rule stays silent — there's no reliable module name to anchor the finding.
