# Architecture

Desktop Foundation combines native compositor configuration, an on-demand
Quickshell interface and Rust command helpers. Niri is the reference compositor;
Hyprland uses the same shell through a compatibility adapter.

The compositor owns window layout, input, outputs, decorations and effects.
Quickshell supplies the launcher, clipboard, Bluetooth controls, session menu,
volume feedback, clock and Niri overview backdrop. Interactive menus are created
when opened and destroyed after their exit animation.

`shell/shell.qml` selects an adapter from `DF_COMPOSITOR`, defaulting to Niri.
Adapters expose normalized windows, workspaces, outputs and capabilities to shared
QML. Niri receives state from its JSON IPC EventStream; Hyprland uses Quickshell's
native integration. Native identifiers and operations stay inside each adapter.
See [the compositor interface](compositor-interface.md).

`native/foundation` builds `desktop-foundationctl` for clipboard storage,
application roles, volume, Bluetooth actions, screenshots, theme generation,
session startup and shell IPC. Commands run on request; long-lived native owners
such as swaybg take over their startup process. `native/nothing` provides a
separate, scoped helper for the [Nothing/CMF device protocol](nothing-controls.md).
See [the command reference](backend.md).

Shared input and window settings live in `config/input.lua` and
`config/window-appearance.lua`. Deployment translates them to native Niri KDL or
Hyprland Lua configuration. Profiles add hardware-specific output or device
settings. The shell palette and application themes come from a published theme
revision under `$XDG_STATE_HOME/desktop-foundation/theme/current`.

Niri's packaged systemd session manages graphical lifecycle, xwayland-satellite
and portal selection. Session startup imports its environment and starts the
polkit agent, clipboard services, notifications and shell. Wallpaper and clipboard
persistence have Niri-bound units; the shell and history watchers stop with the
session. Hyprland supports UWSM and direct startup.

Deployment validates configuration and journals originals before replacing managed
paths. [Installation](installation.md) describes setup;
[recovery](recovery.md) describes restoration.
