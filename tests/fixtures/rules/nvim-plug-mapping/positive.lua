-- Every keymap below binds `<leader>` (or `<localleader>`) as a default
-- in plugin/*.lua without going through `<Plug>` indirection — this
-- takes the key away from users who cannot cleanly remap it.

vim.keymap.set("n", "<leader>ff", function() require("myplugin").find() end)
vim.keymap.set("n", "<Leader>gg", "<cmd>MyPlugin grep<cr>")            -- case-insensitive
vim.keymap.set({ "n", "v" }, "<localleader>xx", "<cmd>MyPlugin x<cr>") -- localleader also flagged
vim.keymap.set("n", "<LocalLeader>yy", function() end)                 -- localleader case-insensitive
