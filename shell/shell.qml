import QtQuick
import Quickshell
import Quickshell.Io
import "adapters/hyprland" as HyprlandBackend
import "surfaces"

ShellRoot {
    id: root
    property bool probeEnabled: false
    HyprlandBackend.Adapter { id: compositor }
    // Only the composition root selects a backend. UI receives the interface.
    LazyLoader {
        active: root.probeEnabled
        Probe { compositor: compositor }
    }
    IpcHandler {
        target: "foundation"
        function showProbe(): void { root.probeEnabled = true; }
        function hideProbe(): void { root.probeEnabled = false; }
        function status(): string {
            return JSON.stringify({ ready: compositor.ready, backend: compositor.backend,
                focusedWindow: compositor.focusedWindow, activeWorkspace: compositor.activeWorkspace,
                windows: compositor.windows, workspaces: compositor.workspaces, probeEnabled: root.probeEnabled });
        }
        function focusWindow(id: string): void { compositor.focusWindow(id); }
        function closeWindow(id: string): void { compositor.closeWindow(id); }
        function focusWorkspace(id: string): void { compositor.focusWorkspace(id); }
        function moveWindow(id: string, workspaceId: string): void { compositor.moveWindow(id, workspaceId); }
    }
}
