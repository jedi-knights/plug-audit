-- Every unguarded require of an optional peer below should fire.

-- Third-party plugin dependencies without pcall
local telescope = require("telescope")
local nio = require("nio")
local snacks = require("snacks.picker")

-- Side-effect require (return value discarded) also fires — the load
-- itself is what fails when the peer is absent, regardless of return use.
require("harpoon").setup({})
