# Niri primary implementation

Implemented and tested against installed Niri 26.04 on Tops. Niri is now the
behavioral reference; Hyprland remains a separate supported secondary backend.
The common UI and visual tokens are shared without a Niri-specific fork.

`compositor/niri/config.kdl` includes native bindings. `scripts/render_niri.py`
renders shared Finnish input and visual intent into the deployed wrapper, with
optional host KDL. `center-focused-column "never"` retains normal scrolling.
The earlier native single-column rule was replaced by a strict single-window
policy: the adapter counts all windows on the focused workspace and issues native
CenterColumn only when exactly one exists and is tiled. A one-shot 80 ms timer
coalesces related events; a workspace/window identity key prevents repeated
centering. Stacked and floating companions both prevent automatic centering.
No coordinates are computed or window positions set; there is no periodic poll.
The policy requires the shell running. Manual Super+C remains available.

`shell/adapters/niri/Adapter.qml` uses two native Quickshell sockets. EventStream
provides initial windows/workspaces and incremental focus, layout, urgency,
keyboard and overview changes. A separate bounded FIFO request connection sends
native actions and output snapshots. Relevant config/workspace events coalesce
output requests. Disconnect clears readiness and state and starts a one-shot
backoff retry; no periodic state poll runs during normal operation.

Output names are opaque IDs; workspace IDs are stable native IDs, distinct from
changing workspace indices. Windows retain nullable native fields, including
geometry coordinates unavailable for tiled windows. Native `FullscreenWindow` is
a toggle. Niri 26.04 IPC does not expose authoritative fullscreen/maximized window
state, so those normalized fields are null and idempotent setters advertise false.
Floating state and explicit tiling/floating requests are implemented. Maximize is
`MaximizeColumn`, not a guessed window maximize flag. Overview, column widths,
centering, directional focus/movement and stable-ID workspace moves use native
Niri actions. Screenshot output/window actions use native Niri IPC with clipboard-only
behavior; region selection uses the repository-styled Wayland slurp/grim driver.
See screenshots.md for drag-release capture and the disabled save path.

Native blur, corner geometry, shadow and border properties implement shared visual
intent. Shadow softness is not numerically equivalent to Hyprland's range/falloff.
Xray blur samples background layers, ignoring other windows. The configured
swaybg wallpaper supplies texture for the blur.
Popup blur remains disabled to protect client menu shapes. Niri's normal opacity
also applies to popup surfaces. Kitty gets compositor opacity 1 and native
background alpha from the shared value, preserving its glyph opacity.

Use [Niri IPC](https://github.com/niri-wm/niri/wiki/IPC),
[26.04 IPC definitions](https://github.com/niri-wm/niri/blob/v26.04/niri-ipc/src/lib.rs),
[native layout](https://niri-wm.github.io/niri/Configuration%3A-Layout.html) and
[window effects](https://niri-wm.github.io/niri/Window-Effects.html).
