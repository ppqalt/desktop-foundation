-- Entry point is loaded by a generated user-local wrapper.
assert(FOUNDATION_ROOT and FOUNDATION_PROFILE, "Run scripts/deploy.py first")
local profile = FOUNDATION_ROOT .. "/profiles/" .. FOUNDATION_PROFILE
for _, name in ipairs({ "hardware", "monitors", "input" }) do
    dofile(profile .. "/" .. name .. ".lua")
end
hl.config({ general = { layout = "scrolling" } })
-- Appearance, animations, blur and gaps use upstream defaults until designed.
local function command(script)
    -- Quote paths for sh, including spaces and single quotes.
    local path = FOUNDATION_ROOT .. "/scripts/" .. script
    return "'" .. path:gsub("'", "'\''") .. "'"
end
hl.on("hyprland.start", function()
    hl.exec_cmd(command("session-start"))
end)
-- TEMPORARY bindings for safe foundation testing; no final key scheme implied.
hl.bind("SUPER + Q", hl.dsp.exec_cmd(command("launch") .. " kitty"))
hl.bind("SUPER + SHIFT + C", hl.dsp.window.close())
hl.bind("SUPER + SHIFT + M", hl.dsp.exec_cmd(command("session-exit")))
for _, direction in ipairs({ "left", "right", "up", "down" }) do
    hl.bind("SUPER + " .. direction, hl.dsp.focus({ direction = direction }))
    hl.bind("SUPER + SHIFT + " .. direction, hl.dsp.window.move({ direction = direction }))
end
for i = 1, 10 do
    hl.bind("SUPER + " .. i % 10, hl.dsp.focus({ workspace = i }))
    hl.bind("SUPER + SHIFT + " .. i % 10, hl.dsp.window.move({ workspace = i }))
end
hl.bind("SUPER + mouse:272", hl.dsp.window.drag(), { mouse = true })
hl.bind("SUPER + mouse:273", hl.dsp.window.resize(), { mouse = true })
hl.bind("SUPER + period", hl.dsp.layout("move +col"))
hl.bind("SUPER + comma", hl.dsp.layout("move -col"))
