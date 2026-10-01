pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "adapters/hyprland" as HyprlandBackend
import "adapters/niri" as NiriBackend
import "surfaces"

ShellRoot {
    id: root
    Volume {
        id: volumeReadout
        targetScreen: root.launcherScreen
    }
    property bool bluetoothEnabled: false
    LazyLoader {
        id: bluetoothLoader
        active: root.bluetoothEnabled
        BluetoothPopup {
            lifecycle: root
            targetScreen: root.launcherScreen
        }
    }
    property bool powerEnabled: false
    LazyLoader {
        id: powerLoader
        active: root.powerEnabled
        Power {
            lifecycle: root
            targetScreen: root.launcherScreen
        }
    }
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
    readonly property var launcherScreen: Quickshell.screens.find(s => s.name === (root.compositorBackend?.outputs ?? []).find(o => o.focused)?.name) ?? Quickshell.screens[0]
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
    readonly property var compositorBackend: backendLoader.item
    Loader {
        id: backendLoader
        sourceComponent: Quickshell.env("DF_COMPOSITOR") === "hyprland" ? hyprlandAdapter : niriAdapter
    }
    Component {
        id: niriAdapter
        NiriBackend.Adapter {}
    }
    Component {
        id: hyprlandAdapter
        HyprlandBackend.Adapter {}
    }
    // Only the composition root selects a backend. UI receives the interface.
    LazyLoader {
        active: root.probeEnabled
        Probe {
            compositor: root.compositorBackend
            lifecycle: root
        }
    }
    IpcHandler {
        target: "foundation"
        function showVolume(percent: string, muted: bool): void {
            volumeReadout.present(Number(percent), muted);
        }
        function volumeStatus(): string {
            return JSON.stringify({
                visible: volumeReadout.visible,
                level: volumeReadout.level,
                muted: volumeReadout.muted
            });
        }
        function toggleBluetooth(): void {
            root.powerEnabled = false;
            if (root.launcher)
                root.launcher.dismiss();
            if (root.clipboard)
                root.clipboard.dismiss();
            if (root.bluetoothEnabled)
                bluetoothLoader.item.dismiss();
            else
                root.bluetoothEnabled = true;
        }
        function bluetoothStatus(): string {
            return JSON.stringify(bluetoothLoader.item ? bluetoothLoader.item.snapshot() : {
                visible: false,
                alive: false
            });
        }
        function togglePower(): void {
            root.bluetoothEnabled = false;
            if (powerLoader.item?.busy)
                return;
            if (root.launcher)
                root.launcher.dismiss();
            if (root.clipboard)
                root.clipboard.dismiss();
            root.powerEnabled = !root.powerEnabled;
        }
        function powerStatus(): string {
            const menu = powerLoader.item;
            return JSON.stringify({
                visible: root.powerEnabled,
                selected: menu?.selected ?? -1,
                busy: menu?.busy ?? false
            });
        }
        function toggleOverview(): void {
            root.compositorBackend.toggleOverview();
        }
        function toggleFullscreen(id: string): void {
            root.compositorBackend.toggleFullscreen(id);
        }
        function maximizeColumn(): void {
            root.compositorBackend.maximizeColumn();
        }
        function cycleColumnWidth(): void {
            root.compositorBackend.cycleColumnWidth();
        }
        function centerColumn(): void {
            root.compositorBackend.centerColumn();
        }
        function toggleFloating(id: string): void {
            root.compositorBackend.toggleFloating(id);
        }
        function moveDirection(direction: string): void {
            root.compositorBackend.moveDirection(direction);
        }
        function screenshotRegion(): void {
            root.compositorBackend.screenshotRegion();
        }
        function screenshotWindow(): void {
            root.compositorBackend.screenshotWindow();
        }
        function screenshotOutput(): void {
            root.compositorBackend.screenshotOutput();
        }
        function toggleClipboard(): void {
            root.bluetoothEnabled = false;
            root.powerEnabled = false;
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
            root.bluetoothEnabled = false;
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
            root.bluetoothEnabled = false;
            root.powerEnabled = false;
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
            root.bluetoothEnabled = false;
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
                ready: root.compositorBackend.ready,
                backend: root.compositorBackend.backend,
                focusedWindow: root.compositorBackend.focusedWindow,
                activeWorkspace: root.compositorBackend.activeWorkspace,
                outputs: root.compositorBackend.outputs,
                capabilities: root.compositorBackend.capabilities,
                eventCount: root.compositorBackend.eventCount,
                backendDiagnostics: {
                    lastError: root.compositorBackend.lastError ?? "",
                    overviewOpen: root.compositorBackend.overviewOpen ?? null,
                    keyboardLayouts: root.compositorBackend.keyboardLayouts ?? null,
                    configFailed: root.compositorBackend.configFailed ?? false
                },
                windows: root.compositorBackend.windows,
                workspaces: root.compositorBackend.workspaces,
                probeAlive: root.probeAlive,
                probeCreations: root.probeCreations,
                probeDestructions: root.probeDestructions,
                probeEnabled: root.probeEnabled
            });
        }
        function focusWindow(id: string): void {
            root.compositorBackend.focusWindow(id);
        }
        function closeWindow(id: string): void {
            root.compositorBackend.closeWindow(id);
        }
        function focusWorkspace(id: string): void {
            root.compositorBackend.focusWorkspace(id);
        }
        function setFullscreen(id: string, enabled: bool): void {
            root.compositorBackend.setFullscreen(id, enabled);
        }
        function setMaximized(id: string, enabled: bool): void {
            root.compositorBackend.setMaximized(id, enabled);
        }
        function setFloating(id: string, enabled: bool): void {
            root.compositorBackend.setFloating(id, enabled);
        }
        function moveWindowToOutput(id: string, outputId: string): void {
            root.compositorBackend.moveWindowToOutput(id, outputId);
        }
        function moveWorkspaceToOutput(id: string, outputId: string): void {
            root.compositorBackend.moveWorkspaceToOutput(id, outputId);
        }
        function scrollColumns(direction: string): void {
            root.compositorBackend.scrollColumns(direction);
        }
        function focusDirection(direction: string): void {
            root.compositorBackend.focusDirection(direction);
        }
        function moveWindow(id: string, workspaceId: string): void {
            root.compositorBackend.moveWindow(id, workspaceId);
        }
    }
}
