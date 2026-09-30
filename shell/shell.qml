pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "adapters/hyprland" as HyprlandBackend
import "surfaces"

ShellRoot {
    id: root
    property bool probeEnabled: false
    property bool probeAlive: false
    property int probeCreations: 0
    property int probeDestructions: 0
    HyprlandBackend.Adapter {
        id: compositorBackend
    }
    // Only the composition root selects a backend. UI receives the interface.
    LazyLoader {
        active: root.probeEnabled
        Probe {
            compositor: compositorBackend
            lifecycle: root
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
                outputs: compositorBackend.outputs,
                capabilities: compositorBackend.capabilities,
                eventCount: compositorBackend.eventCount,
                windows: compositorBackend.windows,
                workspaces: compositorBackend.workspaces,
                probeAlive: root.probeAlive,
                probeCreations: root.probeCreations,
                probeDestructions: root.probeDestructions,
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
        function setFullscreen(id: string, enabled: bool): void {
            compositorBackend.setFullscreen(id, enabled);
        }
        function setMaximized(id: string, enabled: bool): void {
            compositorBackend.setMaximized(id, enabled);
        }
        function setFloating(id: string, enabled: bool): void {
            compositorBackend.setFloating(id, enabled);
        }
        function moveWindowToOutput(id: string, outputId: string): void {
            compositorBackend.moveWindowToOutput(id, outputId);
        }
        function moveWorkspaceToOutput(id: string, outputId: string): void {
            compositorBackend.moveWorkspaceToOutput(id, outputId);
        }
        function scrollColumns(direction: string): void {
            compositorBackend.scrollColumns(direction);
        }
        function focusDirection(direction: string): void {
            compositorBackend.focusDirection(direction);
        }
        function moveWindow(id: string, workspaceId: string): void {
            compositorBackend.moveWindow(id, workspaceId);
        }
    }
}
