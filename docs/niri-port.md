# Future Niri adapter

No Niri implementation is included. Studied 2026-09-30 against the upstream
[IPC guide](https://github.com/niri-wm/niri/wiki/IPC) and
[niri-ipc types](https://niri-wm.github.io/niri/niri_ipc/).

Use `$NIRI_SOCKET`, newline-delimited JSON requests and Ok/Err replies. A dedicated
EventStream connection supplies initial state then incremental changes. Reconnect
with a new initial stream after socket loss; clear readiness and focus immediately.
Ignore unknown additive fields/events and preserve nullable fields. Resource changes
are not transactional: a window can temporarily refer to a removed workspace.
Never guess an output or focus target in that interval.

| Common contract | Niri mapping | Hyprland difference |
|---|---|---|
| Output | name as opaque ID; logical geometry/scale | numeric monitor ID; filter pending placeholder monitors |
| Workspace | stable `id`, optional `name`, `output`, active/focused | named/special workspaces are backend extensions |
| Window | id, title, app_id, workspace_id, is_focused/floating/urgent, layout | hex address and native fullscreen/maximized snapshot |
| Focus | WindowFocusChanged nullable id; WorkspaceActivated.focused | separate native/Wayland initial focus and empty workspace handling |
| Window move | MoveWindowToWorkspace with WorkspaceReferenceArg::Id | never use Niri idx as stable identity |
| Output move | MoveWindowToMonitor / MoveWorkspaceToMonitor | monitor selector stays adapter-local |
| Close/fullscreen | CloseWindow / FullscreenWindow actions | Niri fullscreen request semantics must be checked before exposing set operation |
| Floating | MoveWindowToFloating / MoveWindowToTiling | explicit requests rather than blind toggle |
| Maximize | unsupported common state/request unless exact equivalent added | MaximizeColumn is a column operation, not window maximize |
| Direction/scroll | focus-column/window actions | direction translation is layout-aware |

WorkspacesChanged and WindowsChanged replace complete collections. Apply
WindowOpenedOrChanged, WindowClosed, WindowFocusChanged, WorkspaceActivated,
WorkspaceActiveWindowChanged, WindowLayoutsChanged and urgency updates incrementally.
Outputs may require a request refreshed by output/config events; assess supported
versions rather than assume an output snapshot event exists. Capabilities advertise
only implemented requests, with explicit false/unsupported results elsewhere.

Niri-only extensions: dynamic workspace index, columns/tiles, column width and
full-width state, view offset, consume/expel window, named workspaces and overview.
Hyprland-only extensions: special workspace, client/internal fullscreen divergence,
window groups, scrolling layout dispatch strings. Keep these namespaced, never in
surface code. Both backends expose focus/navigation and optional column-centering/
width requests behind capability checks; the common model does not invent columns
for all compositors.

Future implementation must test initial stream, disconnect/reconnect, unknown
variants, null focus, cross-resource ordering and workspace reindexing. No Rust
daemon is currently justified: direct event-driven QML integration remains the
first option; use a small bridge only if runtime socket parsing demands it.

## Screenshot actions

| Logical action | Hyprland implementation | Planned Niri native implementation |
|---|---|---|
| screenshotRegion() | slurp rectangle, grim region | screenshot interactive UI |
| screenshotWindow() | grim crop of visible focused-window geometry | screenshot-window |
| screenshotOutput() | grim focused output | screenshot-screen |

Translate config/screenshots.toml directory/name intent into Niri screenshot-path.
Prefer Niri's built-in disk and clipboard behavior; do not require it to return an
image to the Hyprland pipeline. Super+Shift+S selects a region and Print captures
the focused output. Window capture has no extra binding. Check native options and
cancellation behavior against the chosen Niri version before advertising support.
