# Foundation architecture

Niri is the primary/reference compositor. Hyprland is supported as a secondary
compatibility backend. Niri defines preferred interaction semantics where the
native models differ; Hyprland approximates them using its own supported layout.
Compositors own layout, input, output, decorations and effects. Quickshell owns the
shared launcher and clipboard surfaces. No permanent bar or Rust daemon exists.

`native/foundation` supplies typed, one-shot system and state operations through
`scripts/foundation`. QML consumes narrow command arguments and the existing
watched clipboard index. The backend does not add a resident service or listener.
`native/nothing` remains independently scoped to the open device-control surface.

The composition root loads one adapter using `DF_COMPOSITOR` (default Niri).
Niri subscribes directly to its JSON IPC EventStream; Hyprland retains native
Quickshell integration. IDs and native operations stay inside adapters. Shared
QML does not run `niri msg` or import Hyprland state. Both surfaces retain the same
components, theme and lazy construction/destruction lifecycle.

Shared `config/input.lua` contains Finnish input; `config/window-appearance.lua`
contains visual intent. Niri deployment renders these to native KDL. Hyprland
translates them to Lua properties. Optional `profiles/<host>/niri.kdl` contains
hardware overrides. Shared configuration has no output name, resolution or GPU.
Native single-column centering belongs to Niri layout, not the shell.

Niri's packaged systemd session owns graphical lifecycle, native
xwayland-satellite integration and portal selection. Session startup imports
Wayland/Niri environment and starts the packaged polkit agent, clipboard watchers
and one runtime-only shell service. Restart limits protect against repeated shell
failure. Basic Kitty, focus and close bindings do not depend on Quickshell.
Hyprland keeps its UWSM/direct startup paths. COSMIC configuration is untouched.

Deployment validates first, journals each original before replacement and refuses
foreign replacement paths during recovery. It does not replace whole directories
or globally enable desktop services. See deployment/recovery and the compositor
interface for details and version-specific limitations.
