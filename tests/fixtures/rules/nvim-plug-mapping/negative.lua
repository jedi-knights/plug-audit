-- None of these keymaps should trip the rule.

-- <Plug> indirection is the correct pattern — user binds their own key
-- to the <Plug> mapping and no default <leader> is stolen.
vim.keymap.set("n", "<Plug>(myplugin-find)", function()
    require("myplugin").find()
end)
vim.keymap.set({ "n", "v" }, "<Plug>(myplugin-selection)", "<cmd>MyPlugin sel<cr>")

-- Direct non-leader keys are out of scope — the rule targets specifically
-- the `<leader>` / `<localleader>` "default keybinding" antipattern.
vim.keymap.set("n", "<C-p>", "<cmd>MyPlugin p<cr>")
vim.keymap.set("n", "gd", "gD")

-- Older API deliberately out of scope for v0.1.0 per rule scope.
vim.api.nvim_set_keymap("n", "<leader>ff", "gg", {})

-- Non-string LHS — dynamic key. Skip.
vim.keymap.set("n", dynamic_key, "<cmd>MyPlugin d<cr>")
