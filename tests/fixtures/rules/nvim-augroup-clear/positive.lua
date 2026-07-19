-- Every augroup below is missing `clear = true` and must be flagged.

vim.api.nvim_create_augroup("MissingOpts")
vim.api.nvim_create_augroup("EmptyTable", {})
vim.api.nvim_create_augroup("ExplicitFalse", { clear = false })
vim.api.nvim_create_augroup("UnrelatedField", { name = "wrong-key" })
