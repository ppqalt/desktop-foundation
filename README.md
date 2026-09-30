# Desktop foundation (temporary name)

A minimal Hyprland + Quickshell/QML foundation for Tops, portable by host profiles and designed for a future Niri adapter. No permanent bar, no production surfaces, no Rust daemon.

```text
compositor/hyprland/    shared Lua configuration and temporary binds
compositor/niri/        interface planning only
shell/adapters/        native compositor integration
shell/components/      reusable presentation components
shell/theme/           temporary probe tokens
shell/surfaces/        removable validation probe only
backend/               justification boundary for future Rust work
profiles/              hardware, monitor and input configuration
scripts/               bootstrap, deploy/restore, launch, checks and measurements
docs/                  architecture, interface, deployment, performance, validation
```

Use the installed **Hyprland (uwsm-managed)** login session for the next login. COSMIC remains in the session chooser. Do not run a second real compositor on the active seat. From a TTY outside a graphical session, `uwsm start hyprland.desktop` is an alternative. The current unmanaged Hyprland session works for validation, but does not prove a complete UWSM login/logout cycle.

Finnish keyboard layout (`fi`) and your supplied iNiR/FEN bindings are active. Super+T or Super+Enter opens Kitty; Super+Q closes; Super+D maximizes; Super+F enters fullscreen; Super+A toggles floating; Super+R cycles normal tiled-column widths; Super+C centers; Super+arrows navigate. Super+E/W use the default file manager/browser. Shared input lives in compositor/hyprland/input.lua, separately from host monitor/GPU configuration.

Overview, launcher, clipboard, settings, cheatsheet and power-menu chords are reserved and inactive until those surfaces exist. Finnish Super+/ is Super+Shift+7 and cannot move a window to workspace 7. The temporary direct session exit remains Super+Shift+M. See [input and binding details](docs/input-bindings.md) for the complete mapping, supplemental controls and real keyboard-event validation. Upstream appearance/blur/animation defaults remain until visual design begins.

From this checkout:

```sh
scripts/deploy.py install --profile Tops
hyprctl reload
scripts/session-start  # only needed in the already-running session
scripts/check
quickshell ipc --path "$PWD/shell" call foundation status
quickshell ipc --path "$PWD/shell" call foundation showProbe
quickshell ipc --path "$PWD/shell" call foundation hideProbe
```

The probe is disabled at startup. Hide destroys it. Quickshell remains resident without a surface. Prefer path selection: a preexisting default Quickshell configuration can shadow named configs. See docs/deployment.md for exact changes and rollback, docs/validation.md for measured results/limits, and docs/compositor-interface.md for the adapter contract.

Upstream references: [Hyprland Lua dispatchers](https://wiki.hypr.land/Configuring/Basics/Dispatchers/), [native scrolling](https://wiki.hypr.land/Configuring/Layouts/Scrolling-Layout/), [Quickshell Hyprland API](https://quickshell.org/docs/v0.3.1/types/Quickshell.Hyprland/Hyprland/), [LazyLoader](https://quickshell.org/docs/v0.3.1/types/Quickshell/LazyLoader/), and installed `uwsm --help`. Installed qmltypes and live tests take precedence when generated online docs differ from package behavior.

Git commits use repository-local `Codex Foundation <codex@localhost>` because no user identity was configured. Change that local identity before your own commits; no global Git settings were changed.
