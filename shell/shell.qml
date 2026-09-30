pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "adapters/hyprland" as HyprlandBackend
import "surfaces"

ShellRoot {
    id: root
    property bool clipboardEnabled: false
    property bool clipboardAlive: false
    readonly property Clipboard clipboard: clipboardLoader.item as Clipboard
    LazyLoader {
        id: clipboardLoader
        active: root.clipboardEnabled
        Clipboard {
            lifecycle: root
            targetScreen: root.launcherScreen
        }
    }
    property bool launcherEnabled: false
    property bool launcherAlive: false
    readonly property Launcher launcher: launcherLoader.item as Launcher
    readonly property var launcherScreen: Quickshell.screens.find(s => s.name === compositorBackend.outputs.find(o => o.focused)?.name) ?? Quickshell.screens[0]
    LazyLoader {
        id: launcherLoader
        active: root.launcherEnabled
        Launcher {
            lifecycle: root
            targetScreen: root.launcherScreen
        }
    }
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
        function toggleClipboard(): void {
            if (root.launcher)
                root.launcher.dismiss();
            if (root.clipboard && !root.clipboard.closing)
                root.clipboard.dismiss();
            else if (root.clipboard)
                root.clipboard.present();
            else
                root.clipboardEnabled = true;
        }
        function showClipboard(): void {
            if (root.launcher)
                root.launcher.dismiss();
            if (root.clipboard)
                root.clipboard.present();
            else
                root.clipboardEnabled = true;
        }
        function hideClipboard(): void {
            if (root.clipboard)
                root.clipboard.dismiss();
        }
        function clipboardQuery(query: string): void {
            if (root.clipboard)
                root.clipboard.setQuery(query);
        }
        function clipboardStatus(): string {
            return JSON.stringify(root.clipboard ? root.clipboard.snapshot() : {
                visible: false
            });
        }
        function toggleLauncher(): void {
            if (root.clipboard)
                root.clipboard.dismiss();
            if (root.launcherEnabled && !root.launcher.closing)
                root.launcher.dismiss();
            else if (root.launcherEnabled)
                root.launcher.present();
            else
                root.launcherEnabled = true;
        }
        function showLauncher(): void {
            if (root.clipboard)
                root.clipboard.dismiss();
            if (root.launcher)
                root.launcher.present();
            else
                root.launcherEnabled = true;
        }
        function hideLauncher(): void {
            if (launcherLoader.item)
                root.launcher.dismiss();
        }
        function launcherQuery(query: string): void {
            if (launcherLoader.item)
                root.launcher.setQuery(query);
        }
        function launcherStatus(): string {
            return JSON.stringify(launcherLoader.item ? root.launcher.snapshot() : {
                visible: false
            });
        }
        function showProbe(): void {
            root.probeEnabled = true;
        }
        function hideProbe(): void {
            root.probeEnabled = false;
        }
        function status(): string {
            return JSON.stringify({
                clipboardAlive: root.clipboardAlive,
                launcherAlive: root.launcherAlive,
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
