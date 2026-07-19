# `nvim/augroup-clear`

**Severity**: Should Fix
**Auto-fix**: no (fix guidance is manual)
**Category**: `nvim/`

## What fires this rule

Any call to `vim.api.nvim_create_augroup` that does not pass `{ clear = true }` as its second positional argument.

Fires on:

```lua
vim.api.nvim_create_augroup("Missing")
vim.api.nvim_create_augroup("EmptyOpts", {})
vim.api.nvim_create_augroup("Explicit", { clear = false })
vim.api.nvim_create_augroup("Other", { name = "wrong-field" })
```

Does not fire on:

```lua
vim.api.nvim_create_augroup("Ok", { clear = true })
vim.api.nvim_create_augroup("Extras", { clear = true, other = 1 })
```

## Why it matters

Neovim reuses an existing augroup ID when `nvim_create_augroup` is called with the same name. Without `clear = true`, every call **appends** its associated autocmds to the group instead of replacing the previous set. Reloading the plugin file at development time (`:source %`) or after `:PackerCompile` / lazy-load re-execution silently accumulates duplicate autocmds — every `BufWritePre` handler fires twice, then three times, then N times. The bug is invisible until an autocmd has an observable side effect, at which point it fires N times in a row.

The `clear = true` flag is the documented Neovim-side switch that says "replace the group on re-creation." It is universally used by every well-maintained plugin in the ecosystem.

## Fix

Add `{ clear = true }` as the second positional argument:

```lua
-- before
vim.api.nvim_create_augroup("Foo")

-- after
vim.api.nvim_create_augroup("Foo", { clear = true })
```

If the second argument is already a table, add the field:

```lua
-- before
vim.api.nvim_create_augroup("Foo", { name = "explicit" })

-- after
vim.api.nvim_create_augroup("Foo", { clear = true, name = "explicit" })
```

## Suppression

Prefer to fix. If a call site legitimately wants the "append" semantics (rare — usually the augroup is created elsewhere with `clear = true`), suppress with an inline reason:

```lua
vim.api.nvim_create_augroup("Foo")  -- plug-audit: disable-line nvim/augroup-clear — group created upstream, appending intentional
```

## Scope

- Applies to every file role. Augroups are equally dangerous in `plugin/`, `lua/`, `after/`, and even test files.
- The qualified form `vim.api.nvim_create_augroup` is required; `local nca = vim.api.nvim_create_augroup` alias detection is out of scope for v0.1.0.
