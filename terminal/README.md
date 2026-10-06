# Terminal environment

Kitty uses Google Sans Code Nerd Font Mono at 11 pt, 5 px padding, a steady beam
cursor and 10,000 lines of scrollback. Its 90% background opacity leaves text
opaque. Colors come from the shared semantic palette.

## Selection

Ctrl+A creates a real Kitty selection across the retained main screen and full
scrollback. Ctrl+C uses `copy_or_interrupt`: copy the selection, or interrupt
when nothing is selected. Escape clears selection and passes Escape to the
application. Mouse selection can replace it.

Kitty has no built-in whole-scrollback selection action. The small `select_all.py`
kitten uses Kitty's native scroll and selection APIs to anchor both endpoints,
then restores the viewing position. On an alternate screen it selects that
screen. These internal APIs require compatibility review when upgrading Kitty.

## Fish

The prompt shows the shortened directory, Git branch, tracked changes and a failed
command's status. Git state refreshes after commands and directory changes.
Native history search, suggestions and bindings remain available.

The abbreviation `c` expands to `clear`. The `fast` function clears the terminal
and runs Fastfetch. Fastfetch is also available directly; it does not run at shell
startup.

## Fastfetch

The package line combines Fastfetch's pacman total with a cached count of
`pacman -Qqm` entries, labelled AUR. Zero remains visible. The foreign count
includes local builds as well as AUR packages.

The Rust backend invalidates that cache when the package databases or repository
configuration change. It avoids cache reuse during a pacman transaction. The
native total is read fresh on each invocation.

## Configuration

`palette.json` supplies the shipped colors. From the repository root, run
`python3 scripts/render_terminal.py` after editing it. Wallpaper theme changes
render the terminal from the active semantic palette.

`scripts/deploy` deploys Kitty, Fish and Fastfetch together with the desktop.
Open a new shell for Fish changes; Kitty supports native configuration reload.
`scripts/dev restore` restores the backed-up desktop and terminal configuration.
See [deployment](../docs/deployment.md) and [theme controls](../theme/README.md).
