# Terminal environment

Clean Niri-first design; no previous Kitty/Fish/Fastfetch settings are imported.
Kitty retains its standard URL opening, clipboard shortcuts, tab behavior and
rendering defaults. Adwaita Mono 11 pt, 5 px padding, steady beam cursor and
10,000 lines of scrollback keep it compact. Native Wayland background opacity
is 0.90; Niri's Kitty rule remains at surface opacity 1.0, preserving glyphs.

Colors live in `palette.json`. Run `python3 scripts/render_terminal.py` from the
repository root after editing it; generated Kitty/Fish/Fastfetch colors are
checked in. No theme generator runs at terminal startup.

Fish retains native history search, suggestions and key bindings. The prompt
shows shortened directory, branch, `*` for tracked changes, failed command
status and a prompt character. Git is refreshed on directory changes and after
commands, never on a prompt repaint. Untracked scans, submodule inspection and
ahead/behind counts are deliberately omitted. Detached checkouts say detached.
Changes made by another process appear after the next command. No framework,
Starship, custom command-not-found hook or inherited aliases are installed.

Run `fastfetch` when wanted. It is intentionally not an automatic greeting:
new terminals and nested shells remain quiet and immediately usable. The normal
Arch logo sits beside aligned, vertically centered groups for the system, session
and hardware. Muted labels keep the logo as the primary accent.
Hostname and hardware are discovered dynamically. Disk uses the
portable root mount, never a machine-specific device name.

Interactive abbreviation `c` expands to `clear`. The native `fast` function
clears the terminal, then runs `fastfetch`. Both live in the tracked Fish
configuration and deploy through the same reversible system on either machine.

## Deployment and rollback

`scripts/deploy` validates and deploys the terminal along with the desktop.
Each original configuration directory is moved intact to the existing state
backup journal before its replacement is linked. Fish's writable universal
variables live under the state directory, outside Git; history in the Fish data
directory is untouched. New Fish sessions use the redesign. Open a new Kitty
window to load its settings; existing terminals are not forcibly restarted.

`scripts/dev restore` restores **all** foundation-owned configuration paths,
including the desktop and these three terminal directories. It refuses to
overwrite paths changed by another tool. Backups and manifest reside at
`${XDG_STATE_HOME:-~/.local/state}/desktop-foundation`. No host-specific terminal
paths, devices or hardware values are embedded in the source configurations.

## Validation on Tops

Twenty separate warm-cache launches before/after replacement:

| Fish | Before median | After median | After p95 |
| --- | ---: | ---: | ---: |
| Noninteractive | 19.78 ms | 12.51 ms | 14.01 ms |
| Interactive configuration | 28.47 ms | 18.43 ms | 20.84 ms |

These measure shell startup (`fish -c exit`, `fish -i -c exit`), not GUI rendering.
In a generated 3,000-file tracked repository, 100 cached prompt repaints took
67.14 ms total (~0.67 ms each). Native Fish syntax checks, Kitty's actual config
loader and Fastfetch's actual parser passed. A real Wayland Kitty was opened,
captured and closed without disturbing existing application windows.
