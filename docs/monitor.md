# System monitor

Super+X opens btop in a dedicated Kitty layer-shell panel on the focused output.
Press it again to close. The full-height panel slides from the right without
moving tiled windows. It accelerates and settles smoothly; toggling during a
slide preserves its position and momentum while reversing direction. It uses
the current wallpaper theme, with transparent rounded corners, a matching blur
and soft shadow. Inner padding keeps the terminal content away from the edges.

btop retains its keyboard controls, including process filtering, menus, Escape
and q. Quitting btop closes its terminal immediately. Use Super+X for an animated
close. No Kitty keyboard grab is used, so desktop shortcuts remain available.

`config/monitor.toml` sets the width, font size, opening and closing durations,
corner radius, padding and btop update interval. The default is 720 logical pixels,
a 10-point font, a 240 ms opening, a 180 ms close, 14-pixel corners, 12-pixel inner
padding and two-second sampling. The panel fills the output's height with no top
or bottom inset, and reaches the right edge. Only the inward-facing corners are
rounded. btop's full layout needs at least
80 columns; use a wider panel or smaller font if its resize notice appears.
Displays below 520×480 logical pixels are rejected. Older configurations without
`closing_animation_ms`, `corner_radius` or `padding` use those defaults.

Corners use a small Kitty custom shader on this panel alone. The packaged
`shader-slang` compiler is an installer dependency; Kitty caches the compiled
effect. The shader has no animation step, so it only runs when the terminal
redraws. Niri rounds the matching blur and shadow. Setting `corner_radius = 0`
disables the shader; normal Kitty windows are unaffected.

Deployment prepares the shader without opening a window. The panel has its own
Kitty cache, so other terminal effects cannot evict it. A small Rust compiler
proxy caches Slang's version query against the installed compiler and library
metadata, and forwards compilation unchanged. Upgrading either invalidates the
version cache. This avoids loading Slang on each warm open, without retaining a
hidden terminal or system sampler.

The isolated configuration is stored in the desktop's runtime directory. btop
saves its layout preferences in `~/.local/state/desktop-foundation/monitor/btop.conf`.
Ordinary Kitty and btop configuration is untouched. New theme revisions are read
on opening; `scripts/monitor refresh` reloads the visible panel's colors.

The Rust controller runs only while the panel is present. It talks to Kitty's
supported panel-position API through a private Unix socket. An eight-millisecond
event wait keeps slides smooth; between slides the controller waits on socket
and process events without a timer. Closing stops btop, Kitty and the controller;
the session-owned user service cleans up their cgroup. Removing the selected
output or changing its logical geometry closes the panel. Reopen it on the new
layout. Existing application focus is released naturally when the layer closes.

Commands: `scripts/monitor toggle`, `close`, `status`, `refresh`, `prepare`. The service is
on demand and is not enabled at login. To disable the feature, remove its Super+X
binding and stop `desktop-foundation-monitor.service`.
