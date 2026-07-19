-- Nothing here should trip the rule.

-- Calling setup itself is fine — the user runs this from their config
-- and this is exactly when setup is expected.
require("myplugin").setup({})

-- Calling a method that does not access .config is fine — methods that
-- work stateless-of-setup are the correct plugin/ surface.
require("myplugin").run()
require("myplugin").info()

-- Local module's own config table (not a require chain) is fine — this
-- is how the module defines its own default state.
local M = {}
M.config = { default = true }

-- Accessing `.config` on an identifier that is not a require chain is
-- also silent — the rule targets specifically the require-chain pattern
-- that indicates cross-module setup dependency.
local cfg = M.config

-- Command callback that never touches require("...").config
vim.api.nvim_create_user_command("MyPluginBar", function()
    require("myplugin").run()
end, {})
