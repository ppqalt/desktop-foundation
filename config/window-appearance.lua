-- Compositor-neutral visual intent. Pixels are logical, colors are #RRGGBB,
-- strengths/opacities are 0..1. Backends translate these into native primitives.
-- Surface-specific QML tokens remain in shell/theme/Theme.qml.
local palette = {}
local source = debug.getinfo(1, "S").source:sub(2)
local path = source:match("^(.*)/config/")
if path then
    local ok, colors = pcall(dofile, path .. "/theme/window-colors.lua")
    if ok and type(colors) == "table" then palette = colors end
end
return {
    rounding = 14,
    spacing = { betweenWindows = 12, desktopEdge = 18 },
    blur = {
        enabled = true,
        radius = 7,
        qualityPasses = 3,
        strength = 1.0,
        noise = 0.012,
        contrast = 1.0,
        brightness = 1.0,
        saturationBoost = 0.06,
        popupEnabled = false,
    },
    shadow = {
        enabled = true,
        color = "#080b10",
        activeOpacity = 0.42,
        inactiveOpacity = 0.12,
        range = 18,
        offset = { x = 0, y = 5 },
        falloffPower = 3,
    },
    focus = {
        inactiveDim = 0.0,
        borderWidth = 1,
        activeBorder = palette.windowActive or "#485669",
        inactiveBorder = palette.windowInactive or "#2b323d",
    },
    opacity = { active = 0.96, inactive = 0.96, fullscreen = 1.0 },
    fullscreen = { decorated = false },
}
