# Small compositor interface

The composition root chooses an adapter and injects it into surfaces. Only shell/adapters/hyprland may import Quickshell.Hyprland. Components must not import it or interpret native IDs. Backend selection is currently explicit at the composition root; add a configuration-selected loader when a second backend exists.

`ready`, `backend`, `capabilities`; reactive `focusedWindow`, `activeWorkspace`, `windows`, `workspaces`. Window records: opaque string `id`, `title`, `appId`, `focused`, nullable `workspaceId`. Workspace records: opaque string `id`, `name`, `active`, `focused` (activeWorkspace contains id/name). No promise of stable IDs across sessions. Null focus and disappearing objects are normal. The adapter initializes Quickshell's standard Wayland toplevel subscription for initial focus and normalizes its native Hyprland event-driven models into records; bindings track property changes, with no timer refresh or subprocess loop.

Requests: `focusWindow(id)`, `closeWindow(id)`, `focusWorkspace(id)`, `moveWindow(id, workspaceId)`. These are asynchronous compositor requests, not confirmation that the operation succeeded; observe the resulting state. IDs must come from current records. Hyprland Lua requests use validated selectors, never interpolate titles or untrusted commands. `toggleOverview()` returns false when unsupported; `capabilities.nativeOverview` is false here. This does not invent an overview UI.

Niri needs its own event subscription and model normalization. Preserve its dynamic workspace IDs and native overview capabilities rather than forcing Hyprland semantics. Add error/result handling when a real interaction surface requires it.

## Hardened records and requests

IDs are opaque strings. Outputs carry id/name/label, logical geometry, scale,
focused and workspaceId. Workspaces carry id/name/outputId, active/focused.
Windows carry id/title/appId/workspaceId/outputId, geometry and nullable state
(fullscreen/maximized/floating/urgent). Focus may be null on an empty workspace or
when no application is focused. Placeholder outputs are excluded. Snapshots of
native window state refresh only on relevant events, coalesced per event-loop turn.
`DF_DEBUG_EVENTS=1` enables a cheap raw-event count exposed over diagnostic IPC;
it is disabled by default and never installs a logging timer.

Requests include focus/close/move window, focus workspace, explicit fullscreen,
maximize and floating state, direction focus, window/workspace output moves.
Selectors are validated in the adapter before native dispatch. `scrollColumns`
is a Hyprland extension that navigates columns by focus; no free viewport-offset
contract is invented. Capabilities must be checked before optional operations.
Lazy probe lifetime counters measure actual object construction/destruction.
