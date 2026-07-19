-- Every access below reads `require("<X>").config` at plugin/ load time
-- (or from a callback that could fire before setup ran). Each one is a
-- setup-dependency that will nil-index if the user's first interaction
-- with the plugin happens before their `setup()` call.

-- Read at load time — the module has not been `setup()`ed yet
local cfg = require("myplugin").config

-- Conditional on a config field — same problem
if require("myplugin").config.enabled then
    vim.notify("myplugin enabled")
end

-- Command callback that reaches into .config on invocation. The command
-- exists whether or not setup() ran; if the user :MyPluginFoo before
-- setup, this nil-indexes.
vim.api.nvim_create_user_command("MyPluginFoo", function()
    local target = require("myplugin").config.target
    vim.notify("running against " .. target)
end, {})

-- Chained deeper — still fires exactly once, on the innermost
-- `require("...").config` access.
local nested = require("myplugin").config.section.field
