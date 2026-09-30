-- Blur only pixels belonging to the launcher panel, not the full-screen scrim.
-- QML owns its short entrance/exit; avoid two independent layer animations.
hl.layer_rule({
    name = "foundation-launcher",
    match = { namespace = "^desktop-foundation-launcher$" },
    blur = true,
    ignore_alpha = 0.4,
    no_anim = true,
})
