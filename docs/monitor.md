# System monitor

Super+X opens btop in a dedicated Kitty layer-shell panel on the focused output.
Press it again to close. The full-height panel slides from the right without
moving tiled windows. It accelerates and settles smoothly; toggling during a
slide preserves its position and momentum while reversing direction. It uses
the current wallpaper theme, with transparent rounded corners, a matching blur
and soft shadow. Inner padding keeps the terminal content away from the edges.

btop retains its keyboard controls, including process filtering, menus, Escape
and q. Quitting btop closes its terminal immediately. Use Super+X for an animated
close. The sidebar uses on-demand keyboard focus once open: click btop to use its
keyboard controls, then click another window or use Niri's window-focus shortcuts
to continue working while it stays visible. Opening does not steal focus; keyboard
input is disabled during the slide. No Kitty keyboard grab or hide-on-focus-loss is used.

`config/monitor.toml` sets the width, font size, opening and closing durations,
corner radius, padding, background opacity and the initial btop update interval. The default is 720 logical pixels,
a 10-point font, a 240 ms opening, a 180 ms close, 14-pixel corners, 12-pixel inner
padding, 75% background opacity and two-second sampling for a new default profile.
Imported or saved btop settings control sampling after that. The panel fills the output's height with no top
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

Kitty's isolated configuration is stored in the desktop's runtime directory.
btop owns a persistent `$XDG_CONFIG_HOME/desktop-foundation/monitor/btop.conf`
(normally `~/.config/desktop-foundation/monitor/btop.conf`). On first deployment
or opening, Desktop Foundation copies `$XDG_CONFIG_HOME/btop/btop.conf`, retaining
its settings and comments. If absent, the former sidebar state file is adopted;
otherwise a default profile is created. The source files remain untouched.

The initial import selects the wallpaper theme, sets `theme_background = false`
and enables `save_config_on_exit`. Later launches never rewrite the profile or
override its update interval. Adjust settings in btop's native menu; q or Super+X
saves them. Disabling btop's save-on-exit option deliberately disables saving.
Kitty's `background_opacity` affects the background alone, retaining opaque text.

New theme revisions are read on opening. `scripts/monitor refresh` updates the
terminal palette without reloading btop's saved settings over unsaved changes;
btop's own theme colors update next time it opens.

The Rust controller runs only while the panel is present. It talks to Kitty's
supported panel-position API through a private Unix socket. An eight-millisecond
event wait keeps slides smooth; between slides the controller waits on socket
and process events without a timer. Closing stops btop, Kitty and the controller.
The controller sends btop SIGINT and keeps its terminal alive for up to two seconds
to save before forced cleanup. The service sends shutdown signals to the controller
first and reserves a final cgroup kill for unresponsive children. Removing the selected
output or changing its logical geometry closes the panel. Reopen it on the new
layout. Existing application focus is released naturally when the layer closes.

Commands: `scripts/monitor toggle`, `close`, `status`, `refresh`, `prepare`. The service is
on demand and is not enabled at login. To disable the feature, remove its Super+X
binding and stop `desktop-foundation-monitor.service`.
