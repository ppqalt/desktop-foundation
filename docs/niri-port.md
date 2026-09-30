# Niri primary implementation

Implemented and tested against installed Niri 26.04 on Tops. Niri is now the
behavioral reference; Hyprland remains a separate supported secondary backend.
The common UI and visual tokens are shared without a Niri-specific fork.

`compositor/niri/config.kdl` includes native bindings. `scripts/render_niri.py`
renders shared Finnish input and visual intent into the deployed wrapper, with
optional host KDL. `always-center-single-column` centers one tiled column, while
`center-focused-column "on-overflow"` supplies normal multi-column scrolling.
Floating windows are excluded from the tiled column count by Niri itself.

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
Niri actions. Screenshot actions delegate directly to native screenshot,
screenshot-window and screenshot-screen, including disk/clipboard ownership.

Native blur, corner geometry, shadow and border properties implement shared visual
intent. Shadow softness is not numerically equivalent to Hyprland's range/falloff.
Xray blur samples only background layers, ignoring other windows. There is no
wallpaper process in the foundation yet; flat gray supplies no visible blur detail.
Popup blur remains disabled to protect client menu shapes. Niri's normal opacity
also applies to popup surfaces. Kitty gets compositor opacity 1 and native
background alpha from the shared value, preserving its glyph opacity.

Use [Niri IPC](https://github.com/niri-wm/niri/wiki/IPC),
[26.04 IPC definitions](https://github.com/niri-wm/niri/blob/v26.04/niri-ipc/src/lib.rs),
[native layout](https://niri-wm.github.io/niri/Configuration%3A-Layout.html) and
[window effects](https://niri-wm.github.io/niri/Window-Effects.html).
