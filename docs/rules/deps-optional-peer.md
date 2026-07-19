# `deps/optional-peer`

**Severity**: Should Fix
**Auto-fix**: no
**Category**: `deps/`

## What fires this rule

Any `require("<target>")` call where `<target>` is not an exempt module (see below) and no ancestor within 10 levels is a `pcall` / `xpcall` function call.

Fires on:

```lua
local telescope = require("telescope")
local nio = require("nio")
local snacks = require("snacks.picker")
require("harpoon").setup({})       -- side-effect require also fires
```

Does not fire on:

```lua
-- Already guarded
local ok, telescope = pcall(require, "telescope")

-- Also guarded — walker sees the pcall ancestor
local ok, m = pcall(function() return require("harpoon") end)

-- Exempt targets
local plenary = require("plenary.async")
local self = require("myplugin.utils")   -- if we are inside "myplugin"
```

## Why it matters

A plugin that lists another plugin as an *optional* integration must degrade silently when the peer is absent. A bare `require("<peer>")` throws a Lua error, and the error propagates all the way out of the plugin bootstrap — the *whole* plugin fails to load, not just the optional integration.

The Lua-era equivalent of tpope's `silent!` idiom is `pcall(require, "X")`: attempt the load, capture success as a boolean, and branch on the boolean to enable or disable the feature.

## Exempt targets

- `vim` and `vim.*` — always available inside Neovim.
- `plenary` and `plenary.*` — universal ecosystem dep per the TODO exemption list.
- The plugin's own module — first-party, always available because we're inside it. Detected from the file's classification (`lua/<name>/...`) or from `RepoContext::primary_module` for `plugin/` files.

## Fix

```lua
-- before
local telescope = require("telescope")
telescope.setup({})

-- after
local ok, telescope = pcall(require, "telescope")
if ok then
    telescope.setup({})
else
    vim.notify("telescope not available, skipping integration", vim.log.levels.DEBUG)
end
```

The `vim.notify` line is optional; silent failure is often the correct choice for genuinely-optional integrations.

## Suppression

Suppress with an inline reason when the require is legitimately required — e.g. the plugin declares this dep as *required* in its README and does not intend to work without it:

```lua
local nio = require("nio")  -- plug-audit: disable-line deps/optional-peer — nio is a hard requirement, documented in README
```

## Scope

- Applies to every file role. `plugin/` bootstrap files are the most common site, but any `.lua` file that runs at load time carries the same risk.
- Variable-target requires (`require(mod_var)`) are silently skipped — the target can't be resolved statically.
- The bare-alias form (`local req = require; req("telescope")`) is out of scope for v0.1.0.
