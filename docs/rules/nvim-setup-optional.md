# `nvim/setup-optional`

**Severity**: Must Fix
**Auto-fix**: no
**Category**: `nvim/`
**Scope**: `plugin/*.lua` files only

## What fires this rule

Any `require("<X>").config` access inside a `plugin/*.lua` file. Detection is a heuristic — access to the `config` field of a required module is the tell that the caller depends on `setup()` having populated it.

Fires on:

```lua
-- plugin/myplugin.lua

-- Read at load time
local cfg = require("myplugin").config

-- Conditional on a config field
if require("myplugin").config.enabled then
    ...
end

-- Command callback reaching into .config
vim.api.nvim_create_user_command("MyPluginFoo", function()
    local target = require("myplugin").config.target
end, {})

-- Chained deeper — fires exactly once on the innermost `.config` link
local nested = require("myplugin").config.section.field
```

Does not fire on:

```lua
-- Calling setup itself
require("myplugin").setup({})

-- Calling any non-config method
require("myplugin").run()

-- Local M.config — not a require chain
local M = {}
M.config = { default = true }
local cfg = M.config
```

## Why it matters

A well-behaved Neovim plugin exposes `setup()` for *configuration*, not for *installation*. The `:MyPluginFoo` command exists whether or not `setup()` ever ran; commands must behave sensibly in either state.

The common failure mode is a `plugin/` command whose callback reaches into `require("myplugin").config.field`. If the user's first interaction with the plugin is `:MyPluginFoo` — before their `require("myplugin").setup({...})` line has executed — the callback nil-indexes and the command fails opaquely.

The invariant is tpope's original one: the plugin's public commands exist without `setup()`. The `<Plug>` layer and the setup-optional contract together form the "well-behaved plugin" surface every user has come to expect.

## Fix

Two idiomatic paths:

**1. Default the config at module top-level.** `require("myplugin").config` is now non-nil even without `setup()`, and `setup(user_opts)` merges into the default:

```lua
-- lua/myplugin/init.lua
local M = {}
M.config = { enabled = true, target = "default" }
function M.setup(opts) M.config = vim.tbl_deep_extend("force", M.config, opts or {}) end
return M
```

**2. Read lazily inside the callback and branch on nil.** Preserves the "setup is a hard prerequisite" story if that's the plugin's design:

```lua
vim.api.nvim_create_user_command("MyPluginFoo", function()
    local ok, mp = pcall(require, "myplugin")
    if not ok or not mp.config then
        vim.notify("myplugin: run require('myplugin').setup({...}) first", vim.log.levels.ERROR)
        return
    end
    do_thing(mp.config.target)
end, {})
```

## Known false positives (v0.1.0 scope)

- `require("plenary").config` and similar universal-peer targets — plenary genuinely maintains its own config lazily. Exempting these would need a target-based allow list; deferred to v0.2.0.
- Access inside a `pcall`- or `if <ok>`-guarded branch — the rule does not walk ancestors to detect this pattern.

Both are documented limitations. The Must Fix severity reflects that the failure the rule prevents (opaque nil-index at command invocation) is far more painful than a false-positive noisy finding, and the fix is small.

## Suppression

Legitimate suppress site: a plugin that documents "you must call `setup()` before any command" as a hard contract and enforces it in the callback:

```lua
vim.api.nvim_create_user_command("MyPluginFoo", function()
    if require("myplugin").config.target then    -- plug-audit: disable-line nvim/setup-optional — nil-branch on next line handles pre-setup state
        do_thing()
    end
end, {})
```

## Scope

- Only `plugin/*.lua`. Access to `.config` from `lua/<name>/*.lua` is legitimate module-internal state.
- Only string-literal require targets. Dynamic `require(mod_var).config` is silently skipped.
- Fires on the *innermost* `require(...).config` in a chain — `require("m").config.a.b` produces one finding, not two.
