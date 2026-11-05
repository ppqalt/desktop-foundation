# Window appearance

Shared settings are in `config/window-appearance.toml`. The Niri renderer produces
native KDL.

The default uses 14-pixel corners, 90% window opacity and narrow focus borders.
Kitty uses its own background opacity so text remains opaque.
Niri centers a single tiled column; multiple columns retain scrolling behavior.
Super+C centers the current column manually.

Shortcut menus use graphite cards and dimmed backgrounds. Their blur samples
underlying applications. Normal windows retain the configured wallpaper blur.
Fullscreen uses native output geometry and opaque content; popup effects have separate rules.

Output settings belong in `profiles/NAME/niri.kdl`. The default profile selects
outputs automatically; `dual-display-example` provides an editable layout.

The renderer checks for edits to the old `config/input.lua` and
`config/window-appearance.lua` before using the TOML files. If either was
customized, generation stops without changing it. Copy the customized values to
the corresponding TOML file (or the input user override), then move the old Lua
file aside and retry. Unchanged legacy files are ignored by the native renderer.
