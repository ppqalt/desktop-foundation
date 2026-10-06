# Window appearance

Shared settings are in `config/window-appearance.lua`. The Niri renderer produces
native KDL; Hyprland reads the same values through Lua.

The default uses 14-pixel corners, 96% window opacity, narrow focus borders and
soft shadows. Kitty uses its own background opacity so text remains opaque.
Niri centers a single tiled column; multiple columns retain scrolling behavior.
Super+C centers the current column manually.

Shortcut menus use graphite cards and dimmed backgrounds. Their blur samples
underlying applications. Normal windows retain the configured wallpaper blur.
Fullscreen uses native output geometry; popup effects have separate rules.

Output settings belong in `profiles/NAME/niri.kdl`. The default profile selects
outputs automatically; `dual-display-example` provides an editable layout.
