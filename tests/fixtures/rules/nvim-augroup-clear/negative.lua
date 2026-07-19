-- Every augroup below either passes `clear = true` explicitly or is a
-- lookalike call the rule must NOT match.

vim.api.nvim_create_augroup("Explicit", { clear = true })
vim.api.nvim_create_augroup("ExtraFields", { clear = true, other = 1 })

-- Not a create_augroup call — the rule must ignore `nvim_del_augroup`
-- and any other identifier whose text merely contains "augroup".
vim.api.nvim_del_augroup_by_name("Something")
local id = vim.api.nvim_create_autocmd("BufWritePost", { group = 1, callback = function() end })

-- Not the vim.api form — user's own wrapper. Out of scope for v0.1.0.
local mine = require("mine.augroup").create("Foo")
