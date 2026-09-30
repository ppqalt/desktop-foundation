# Small compositor interface

The shell root loads one adapter and injects normalized state into shared surfaces.
Default `DF_COMPOSITOR` is niri; hyprland explicitly selects the retained backend.
Only adapter-local code interprets native IDs or executes native operations.

Reactive properties: ready, backend, capabilities, outputs, workspaces, windows,
focusedWindow, activeWorkspace/focusedWorkspace. IDs are opaque strings and are
not stable across sessions. Workspace records include name/index/output/active/focus;
window records include app/title/workspace/output/focus, nullable geometry and
nullable fullscreen/maximized/floating/urgent state. Null focus and cross-resource
ordering during removal are normal. Never infer fullscreen from window size.

Asynchronous requests: focus/close window, focus workspace, move window to stable
workspace ID, window/workspace output moves, explicit floating state, direction
focus/movement, overview toggle, column width cycle/centering/maximize, fullscreen
toggle and screenshot region/window/output. Observe events to confirm results.
Check capabilities before optional requests. Niri fullscreen/maximized state is
unavailable in 26.04 IPC, so those fields are null and idempotent setters return
false. Its native maximize action is a column operation. Hyprland retains its
native state/setters and advertises no native overview.

Niri's EventStream supplies initial and incremental state, with one-shot reconnect
backoff only on failure. Output requests coalesce on relevant events. Action requests
use a bounded separate FIFO. Hyprland uses native Quickshell event subscriptions
and coalesced relevant refreshes. No periodic resident subprocess/state poll exists.
Niri layout and focus timestamps remain namespaced under `niri` in window records.
Screenshot UX/storage is backend-owned, using common config path intent.
