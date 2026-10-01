-- iNiR/FEN muscle memory. No shell surfaces are implemented by this file.
return function(command)
    local function bind(key, action, description)
        hl.bind(key, action, { description = description })
    end
    for _, key in ipairs({ "T", "Return" }) do
        bind("SUPER + " .. key, hl.dsp.exec_cmd(command("launch") .. " kitty"), "Open Kitty")
    end
    bind("SUPER + E", hl.dsp.exec_cmd(command("launch") .. ' xdg-open "$HOME"'), "Open default file manager")
    bind("SUPER + W", hl.dsp.exec_cmd(command("launch") .. " xdg-open about:blank"), "Open default browser")
    bind("SUPER + Q", hl.dsp.window.close(), "Close focused window")
    bind("SUPER + D", hl.dsp.window.fullscreen({ mode = "maximized" }), "Toggle maximize")
    bind("SUPER + F", hl.dsp.window.fullscreen({ mode = "fullscreen" }), "Toggle fullscreen")
    bind("SUPER + A", hl.dsp.window.float({ action = "toggle" }), "Toggle floating")
    -- Use native preset widths; do not guess the absent original FEN presets.
    bind("SUPER + R", hl.dsp.layout("colresize +conf"), "Cycle tiled column width")
    bind("SUPER + C", function()
        local window = hl.get_active_window()
        if not window then return end
        if window.floating then
            hl.dispatch(hl.dsp.window.center())
        else
            hl.dispatch(hl.dsp.layout("center"))
        end
    end, "Center floating window or tiled column")
    for _, direction in ipairs({ "left", "right", "up", "down" }) do
        bind("SUPER + " .. direction, hl.dsp.focus({ direction = direction }), "Focus " .. direction)
        bind("SUPER + SHIFT + " .. direction, hl.dsp.window.move({ direction = direction }), "Move window " .. direction)
    end

    bind("SUPER + space", hl.dsp.exec_cmd(command("launcher")), "Toggle application launcher")

    bind("SUPER + B", hl.dsp.exec_cmd(command("bluetooth-popup")), "Paired Bluetooth devices")
    bind("SUPER + V", hl.dsp.exec_cmd(command("clipboard")), "Toggle clipboard history")

    bind("SUPER + SHIFT + S", hl.dsp.exec_cmd(command("screenshot") .. " --backend hyprland region"), "Screenshot region: save and copy")
    bind("Print", hl.dsp.exec_cmd(command("screenshot") .. " --backend hyprland output"), "Screenshot focused output: save and copy")

    bind("SUPER + SHIFT + Q", hl.dsp.exec_cmd(command("power-menu")), "Power/session menu")

    -- Reserved, deliberately inert until the corresponding UI is designed.
    -- Hyprland does not supply a built-in Niri-style overview.
    local reserved = {
        { "SUPER + Tab", "overview" },
        { "SUPER + comma", "settings/control UI" },
        -- Finnish slash = Shift+7. XKB code 16 is <AE07> (evdev code 8 + 8).
        -- Keycode binding avoids shifted-keysym/modifier ambiguity.
        { "SUPER + SHIFT + code:16", "cheatsheet (Finnish Super+/)" },
    }
    for _, entry in ipairs(reserved) do
        bind(entry[1], hl.dsp.no_op(), "Reserved: " .. entry[2] .. " (not implemented)")
    end

    -- Temporary supplemental foundation controls, outside the requested keys.
    bind("SUPER + SHIFT + M", hl.dsp.exec_cmd(command("session-exit")), "Temporary direct session exit")
    for i = 1, 10 do
        bind("SUPER + " .. i % 10, hl.dsp.focus({ workspace = i }), "Temporary focus workspace " .. i)
        -- Shift+7 belongs to Super+/; do not move a window to workspace 7.
        if i ~= 7 then
            bind("SUPER + SHIFT + " .. i % 10, hl.dsp.window.move({ workspace = i }), "Temporary move to workspace " .. i)
        end
    end
    bind("SUPER + mouse:272", hl.dsp.window.drag(), "Move window with mouse")
    bind("SUPER + mouse:273", hl.dsp.window.resize(), "Resize window with mouse")
    bind("SUPER + period", hl.dsp.layout("move +col"), "Temporary scroll forward one column")
    bind("SUPER + ALT + comma", hl.dsp.layout("move -col"), "Temporary scroll backward one column")
end
