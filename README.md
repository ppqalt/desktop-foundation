# Desktop foundation

Niri 26.04 is the primary compositor and the reference for interaction. Hyprland
remains a supported secondary backend. The shared Quickshell launcher and clipboard
manager retain the approved graphite glass design, load only on demand and leave
no permanent bar. COSMIC is untouched.

```text
compositor/niri/        native bindings and modular Niri configuration
compositor/hyprland/    retained Lua compatibility backend
config/                shared Finnish input, visual intent and screenshot storage
shell/adapters/        compositor-local event and action translation
shell/components/      reusable presentation components
shell/theme/           shared graphite glass tokens
shell/surfaces/        lazy launcher, clipboard and diagnostic probe
profiles/              optional machine-specific monitor/GPU/device configuration
scripts/               deployment, session startup, recovery and checks
docs/                  architecture, behavior and validation
```

Select **Niri** at login on Tops. From this checkout:

```sh
scripts/deploy.py install --compositor niri --profile Tops
scripts/session-start  # for an already-running session
scripts/check
scripts/doctor
```

Niri reloads its configuration automatically. Deployment validates native KDL
before replacing only its owned config path; originals are backed up. Optional
`profiles/<host>/niri.kdl` contains hardware overrides. Without it the configuration
is portable, including on lucky38. Shared Finnish input is in `config/input.lua`.

Super+Tab opens Niri overview; Super+Space opens the launcher; Super+V opens the
clipboard. Selecting history copies it and closes the surface. Super+T/Enter opens
Kitty; Super+E/W opens the default file manager/browser. Super+Q closes, D maximizes
the column, F toggles fullscreen, A toggles floating, R cycles column widths and C
centers the column. Arrows/HJKL focus; Shift variants move. Super+1..9 switches
workspace; Ctrl variants move the window. Finnish Super+/ is Super+Shift+7.
Settings, cheatsheet and power menu remain reserved. Print captures the current
output; Super+Shift+S uses Niri's native region UI. PageUp/Down changes volume 3%.

A workspace with one tiled column is centered by Niri's native
`always-center-single-column`; multiple columns retain normal scrolling. There is
no custom positioning loop. Active/inactive window opacity is equally **90% for
the current trial**. Kitty uses native background transparency with opaque text.
Native blur currently samples the wallpaper/background; a uniform gray background
cannot visibly demonstrate its blur kernel.

For Hyprland use `scripts/deploy.py install --compositor hyprland --profile Tops`
and choose its UWSM login session. Both configs can remain deployed. Switching
configuration does not switch the running compositor. `scripts/deploy.py restore`
restores all foundation-owned config paths. See [deployment](docs/deployment.md),
[recovery](docs/recovery.md), [bindings](docs/input-bindings.md),
[Niri implementation](docs/niri-port.md) and [validation](docs/niri-validation.md).

No Rust daemon or extra desktop feature has been added. Git uses repository-local
`Codex Foundation <codex@localhost>`; no global identity setting was changed.

## Terminal

The portable Kitty, native Fish prompt and on-demand Fastfetch configuration live
in `terminal/`. See [terminal documentation](terminal/README.md) for palette,
startup measurements and reversible deployment.
