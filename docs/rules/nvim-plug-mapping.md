# `nvim/plug-mapping`

**Severity**: Should Fix
**Auto-fix**: no
**Category**: `nvim/`
**Scope**: `plugin/*.lua` files only

## What fires this rule

Any `vim.keymap.set` call inside a `plugin/*.lua` file whose LHS (2nd positional argument) is a string literal starting with `<leader>` or `<localleader>` (case-insensitive).

Fires on:

```lua
-- plugin/myplugin.lua
vim.keymap.set("n", "<leader>ff", function() require("myplugin").find() end)
vim.keymap.set("n", "<Leader>gg", "<cmd>MyPlugin grep<cr>")
vim.keymap.set({ "n", "v" }, "<localleader>xx", "<cmd>MyPlugin x<cr>")
```

Does not fire on:

```lua
-- <Plug> indirection — the correct pattern
vim.keymap.set("n", "<Plug>(myplugin-find)", function() end)

-- Non-leader keys (out of scope for this rule)
vim.keymap.set("n", "<C-p>", "<cmd>MyPlugin p<cr>")
vim.keymap.set("n", "gd", "gD")

-- Older API (out of scope for v0.1.0)
vim.api.nvim_set_keymap("n", "<leader>ff", "gg", {})

-- Dynamic LHS (can't resolve statically — skip)
vim.keymap.set("n", dynamic_key, ...)
```

Also does not fire in `lua/<name>/**/*.lua`, `after/**`, or test files — only `plugin/*.lua`.

## Why it matters

tpope's `<Plug>`-first idiom, now the Neovim ecosystem standard, exists because default `<leader>` keymaps create two compounding problems:

1. **Conflict landscape.** Two plugins that both ship `<leader>ff` cannot coexist without the user disabling one of the defaults. The number of plugins that ship `<leader>` bindings grows every year; conflict is inevitable.
2. **Remapping friction.** Users who want a different key have to first *disable* the default (via `del_keymap` or an autocmd) and then re-bind. The plugin does not know the user's preferred key space.

The `<Plug>` layer solves both:

- The plugin exposes `<Plug>(myplugin-find)` — a mapping the user cannot accidentally trigger from the keyboard.
- The user binds their preferred key (`<leader>ff`, `<C-p>`, whatever) to the `<Plug>` mapping in their config.
- Conflict resolution moves from "which plugin loaded last" to "which key did the user choose."

## Fix

Split the shipped mapping from the user's default binding:

```lua
-- plugin/myplugin.lua
vim.keymap.set("n", "<Plug>(myplugin-find)", function()
    require("myplugin").find()
end)
```

Document the recommended user binding in the README:

```markdown
## Suggested keybindings

```lua
vim.keymap.set("n", "<leader>mf", "<Plug>(myplugin-find)")
```
```

## Suppression

Rare, but occasionally the plugin *is* the leader-key convention (e.g. `which-key` shipping a diagnostic mapping). Suppress at the call site with an inline reason:

```lua
vim.keymap.set("n", "<leader>?", function() require("myplugin").help() end)
-- plug-audit: disable-line nvim/plug-mapping — help lookup, README documents no user rebinding needed
```

## Scope notes

- Only `vim.keymap.set`. The older `vim.api.nvim_set_keymap` API is deliberately out of scope for v0.1.0 — its user base is shrinking, and matching both doubles the detection surface for marginal signal.
- Only `plugin/*.lua` files. `lua/<name>/*.lua` and `after/*.lua` files are not shipped by default at load time and do not have the same conflict story.
- Only string-literal LHSes are inspected. `vim.keymap.set(mode, dynamic_key, ...)` is silently skipped — the target can't be resolved statically.
- Case-insensitive on the `<leader>` / `<localleader>` prefix — Vim treats these tokens case-insensitively.
