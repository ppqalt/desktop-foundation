-- Entry point is loaded by a generated user-local wrapper.
assert(FOUNDATION_ROOT and FOUNDATION_PROFILE, "Run scripts/deploy.py first")
dofile(FOUNDATION_ROOT .. "/compositor/hyprland/input.lua")
local profile = FOUNDATION_ROOT .. "/profiles/" .. FOUNDATION_PROFILE
for _, name in ipairs({ "hardware", "monitors", "input" }) do
    dofile(profile .. "/" .. name .. ".lua")
end
hl.config({ general = { layout = "scrolling" } })
-- Appearance, animations, blur and gaps use upstream defaults until designed.
local function command(script)
    -- Quote paths for sh, including spaces and single quotes.
    local path = FOUNDATION_ROOT .. "/scripts/" .. script
    return "'" .. path:gsub("'", "'\\''") .. "'"
end
hl.on("hyprland.start", function()
    hl.exec_cmd(command("session-start"))
end)
local configureBindings = dofile(FOUNDATION_ROOT .. "/compositor/hyprland/bindings.lua")
configureBindings(command)
