-- Translate shared intent here; common config contains no hl.* or rule selectors.
local visual = dofile(FOUNDATION_ROOT .. "/config/window-appearance.lua")
local function rgba(color, alpha)
    local rgb = color:gsub("^#", "")
    return string.format("rgba(%s%02x)", rgb, math.floor(alpha * 255 + 0.5))
end
local blur, shadow, focus = visual.blur, visual.shadow, visual.focus
hl.config({
    general = {
        gaps_in = visual.spacing.betweenWindows / 2,
        gaps_out = visual.spacing.desktopEdge,
        border_size = focus.borderWidth,
        col = {
            active_border = rgba(focus.activeBorder, 1),
            inactive_border = rgba(focus.inactiveBorder, 1),
        },
    },
    decoration = {
        rounding = visual.rounding,
        active_opacity = visual.opacity.active,
        inactive_opacity = visual.opacity.inactive,
        fullscreen_opacity = visual.opacity.fullscreen,
        dim_inactive = focus.inactiveDim > 0,
        dim_strength = focus.inactiveDim,
        blur = {
            enabled = blur.enabled and blur.strength > 0,
            size = math.max(1, math.floor(blur.radius * blur.strength + 0.5)),
            passes = blur.qualityPasses,
            -- No separate native blur alpha. Strength scales the kernel radius;
            -- keep application opacity independent of this quality treatment.
            ignore_opacity = true,
            new_optimizations = true,
            xray = false,
            noise = blur.noise,
            contrast = blur.contrast,
            brightness = blur.brightness,
            vibrancy = blur.saturationBoost,
            popups = blur.popupEnabled,
        },
        shadow = {
            enabled = shadow.enabled,
            color = rgba(shadow.color, shadow.activeOpacity),
            color_inactive = rgba(shadow.color, shadow.inactiveOpacity),
            range = shadow.range,
            render_power = shadow.falloffPower,
            offset = { shadow.offset.x, shadow.offset.y },
            sharp = false,
        },
        glow = { enabled = false },
    },
})
if not visual.fullscreen.decorated then
    -- Match internal true fullscreen (2), not scrolling-layout maximize (1).
    hl.window_rule({
        name = "foundation-clean-fullscreen",
        match = { fullscreen_state_internal = 2 },
        border_size = 0,
        no_shadow = true,
        rounding = 0,
    })
end
