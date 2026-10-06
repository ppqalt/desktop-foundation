# Compositor interface

`shell/shell.qml` selects one adapter and injects its state into shared surfaces.
`DF_COMPOSITOR=hyprland` selects Hyprland; the default is Niri. Adapter code owns
native identifiers, IPC messages and operation mapping.

## State

| Property | Meaning |
|---|---|
| `ready`, `backend`, `capabilities` | Connection readiness, adapter name and supported operations |
| `outputs` | Connected outputs and focus information |
| `workspaces` | ID, name/index, output, active and focused state |
| `windows` | ID, app/title, workspace/output, focus, geometry and native state |
| `focusedWindow` | Focused window, or null |
| `activeWorkspace`, `focusedWorkspace` | Focused workspace, or null |

IDs are opaque strings whose lifetime is one compositor session. Geometry and
fullscreen/maximized/floating/urgent fields may be null when unavailable.
Removal events can briefly leave related records out of order. Consumers should
handle null focus and check capabilities before optional operations.

## Requests

Adapters support window focus/close, workspace focus, moving a window to a stable
workspace ID, window/workspace output moves, floating state, directional focus
and movement, column width cycling/centering/maximization, fullscreen toggling
and region/window/output screenshots. Requests are asynchronous; events supply
the resulting state.

Niri's adapter exposes native overview and column operations. Its 26.04 IPC does
not supply authoritative fullscreen/maximized state: those normalized fields
are null, and `setFullscreen`/`setMaximized` return false. `toggleFullscreen` and
`maximizeColumn` remain native actions. Hyprland exposes its native setters and
reports no overview capability. Screenshot storage follows the selected backend.

Niri uses one JSON IPC EventStream for initial and incremental state. Relevant
events coalesce output refreshes. A separate request queue holds at most 64
requests, with a three-second deadline per request; a timed-out action is not
replayed. Connection failures trigger bounded backoff before a fresh snapshot.
Niri-specific layout and focus data stays under the `niri` namespace in window
records. Hyprland uses native Quickshell subscriptions and coalesced refreshes.

The Niri adapter also centers a focused tiled window when it is the sole window
on the focused workspace. Floating companions or additional windows prevent
this policy from running; `Super+C` centers a column explicitly.
