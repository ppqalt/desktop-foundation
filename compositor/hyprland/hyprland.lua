-- Entry point is loaded by a generated user-local wrapper.
assert(FOUNDATION_ROOT and FOUNDATION_PROFILE, "Run scripts/deploy.py first")
dofile(FOUNDATION_ROOT .. "/compositor/hyprland/input.lua")
local profile = FOUNDATION_ROOT .. "/profiles/" .. FOUNDATION_PROFILE
for _, name in ipairs({ "hardware", "monitors", "input" }) do
    local path = profile .. "/" .. name .. ".lua"
    local file = io.open(path, "r")
    if file then
        file:close()
    else
        path = FOUNDATION_ROOT .. "/profiles/default/" .. name .. ".lua"
    end
    dofile(path)
end
hl.config({ general = { layout = "scrolling" } })
dofile(FOUNDATION_ROOT .. "/compositor/hyprland/animations.lua")
-- Appearance, blur and gaps use upstream defaults until designed.
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
