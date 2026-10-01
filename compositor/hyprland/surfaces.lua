-- Blur only pixels belonging to the launcher panel, not the full-screen scrim.
-- QML owns its short entrance/exit; avoid two independent layer animations.
hl.layer_rule({
    name = "foundation-launcher",
    match = { namespace = "^desktop-foundation-(launcher|clipboard|power|bluetooth)$" },
    blur = true,
    ignore_alpha = 0.4,
    no_anim = true,
})

-- XWayland menus carry transparent shadow margins. Blurring these floating
-- surfaces turns the margins into a visible rectangle (Brave and ChatGPT).
-- Keep glass blur on the shell and normal tiled application windows.
hl.window_rule({
    name = "foundation-xwayland-floating-no-blur",
    match = { xwayland = true, float = true },
    no_blur = true,
})
