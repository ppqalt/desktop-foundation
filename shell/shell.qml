pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "adapters/hyprland" as HyprlandBackend
import "surfaces"

ShellRoot {
    id: root
    property bool probeEnabled: false
    HyprlandBackend.Adapter {
        id: compositorBackend
    }
    // Only the composition root selects a backend. UI receives the interface.
    LazyLoader {
        active: root.probeEnabled
        Probe {
            compositor: compositorBackend
        }
    }
    IpcHandler {
        target: "foundation"
        function showProbe(): void {
            root.probeEnabled = true;
        }
        function hideProbe(): void {
            root.probeEnabled = false;
        }
        function status(): string {
            return JSON.stringify({
                ready: compositorBackend.ready,
                backend: compositorBackend.backend,
                focusedWindow: compositorBackend.focusedWindow,
                activeWorkspace: compositorBackend.activeWorkspace,
                windows: compositorBackend.windows,
                workspaces: compositorBackend.workspaces,
                probeEnabled: root.probeEnabled
            });
        }
        function focusWindow(id: string): void {
            compositorBackend.focusWindow(id);
        }
        function closeWindow(id: string): void {
            compositorBackend.closeWindow(id);
        }
        function focusWorkspace(id: string): void {
            compositorBackend.focusWorkspace(id);
        }
        function moveWindow(id: string, workspaceId: string): void {
            compositorBackend.moveWindow(id, workspaceId);
        }
    }
}
