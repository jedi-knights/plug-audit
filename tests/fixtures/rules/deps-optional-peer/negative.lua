-- No require in this file should trip the rule.

-- Guarded — pcall(require, "X") form. No nested require function_call
-- exists, so nothing for the rule to inspect.
local ok, telescope = pcall(require, "telescope")

-- Guarded — pcall around a function that calls require. The rule walks
-- ancestors to find the pcall function_call and exempts the inner require.
local ok2, m = pcall(function()
    return require("harpoon")
end)

-- xpcall variant.
local ok3, m2 = xpcall(function()
    return require("nio")
end, debug.traceback)

-- Exempt targets — vim, plenary, and first-party (self module).
local plenary = require("plenary.async")
local first_party = require("myplugin.utils")
local first_party_root = require("myplugin")
