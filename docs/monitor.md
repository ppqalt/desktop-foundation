# System monitor

Super+X opens btop in a dedicated Kitty layer-shell panel on the focused output.
Press it again to close. The panel slides from the right without moving tiled
windows; toggling during a slide reverses it. It uses the current wallpaper theme.

btop retains its keyboard controls, including process filtering, menus, Escape
and q. Quitting btop closes its terminal immediately. Use Super+X for an animated
close. No Kitty keyboard grab is used, so desktop shortcuts remain available.

`config/monitor.toml` sets the width, font size, animation duration and btop update
interval. The default is 720 pixels, a 10-point font, a 180 ms slide and two-second
sampling. btop's full layout needs at least 80 columns; use a wider panel or smaller
font if its resize notice appears. Displays below 520×480 logical pixels are rejected.

The isolated configuration is stored in the desktop's runtime directory. btop
saves its layout preferences in `~/.local/state/desktop-foundation/monitor/btop.conf`.
Ordinary Kitty and btop configuration is untouched. New theme revisions are read
on opening; `scripts/monitor refresh` reloads the visible panel's colors.

The Rust controller runs only while the panel is present. It talks to Kitty's
supported panel-position API through a private Unix socket and waits on socket
and process events between slides. Closing stops btop, Kitty and the controller;
the session-owned user service cleans up their cgroup. Removing the selected
output or changing its logical geometry closes the panel. Reopen it on the new
layout. Existing application focus is released naturally when the layer closes.

Commands: `scripts/monitor toggle`, `close`, `status`, `refresh`. The service is
on demand and is not enabled at login. To disable the feature, remove its Super+X
binding and stop `desktop-foundation-monitor.service`.
