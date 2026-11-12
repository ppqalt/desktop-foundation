pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "adapters/niri" as NiriBackend
import "surfaces"
import "components"
import "theme"
import "services" as Services

ShellRoot {
    id: root
    ReloadNotice {}
    Component.onCompleted: Quickshell.watchFiles = false
    Services.MediaControl {
        id: mediaControl
    }
    Services.RecordingState {
        id: recordingState
    }
    LazyLoader {
        id: backdropLoader
        active: Quickshell.env("DF_COMPOSITOR") === "niri"
        OverviewBackdrop {}
    }
    Volume {
        id: volumeReadout
        targetScreen: root.launcherScreen
    }
    ModalBackdrop {
        targetScreen: root.launcherScreen
        requested: root.popupBackdropRequested
    }
    readonly property bool popupBackdropRequested: (root.launcherEnabled && !root.launcher?.superseded && !root.launcher?.closing) || (root.clipboardEnabled && !root.clipboard?.superseded && !root.clipboard?.closing) || (root.bluetoothEnabled && !bluetoothLoader.item?.superseded && !bluetoothLoader.item?.closing) || (root.powerEnabled && !powerLoader.item?.superseded && !powerLoader.item?.closing) || ["launcher", "clipboard", "bluetooth", "power"].includes(surfaceRouter.pending)
    ClockVisibility {
        id: clockVisibility
        recordingPhase: recordingState.phase
    }
    property alias clockEnabled: clockVisibility.enabled
    readonly property bool clockVisible: clockVisibility.visible
    LazyLoader {
        id: clockLoader
        active: root.clockVisible
        Clock {
            targetScreen: root.launcherScreen
            recordingPhase: recordingState.phase
            replayActive: recordingState.replayActive
        }
    }
    readonly property ControlCenter controlCenter: controlCenterLoader.item as ControlCenter
    property bool controlCenterEnabled: false
    Services.ControlCenterState {
        id: controlCenterState
        live: true
    }
    property var controlCenterStyle: ({
            width: 720,
            padding: 12,
            radius: 14,
            opacity: 0.75,
            openingMs: 240,
            closingMs: 180
        })
    LazyLoader {
        id: controlCenterLoader
        active: root.controlCenterEnabled
        ControlCenter {
            rememberedState: controlCenterState
            recording: recordingState
            handoff: root.handoffSurface === "controlCenter"
            sidebarStyle: root.controlCenterStyle
            targetScreen: root.launcherScreen
            onDismissed: nextSurface => {
                root.controlCenterEnabled = false;
                if (nextSurface === "bluetooth")
                    surfaceRouter.request("bluetooth", true);
            }
        }
    }
    property bool bluetoothEnabled: false
    LazyLoader {
        id: bluetoothLoader
        active: root.bluetoothEnabled
        BluetoothPopup {
            sharedBackdrop: true
            handoff: root.handoffSurface === "bluetooth"
            lifecycle: root
            targetScreen: root.launcherScreen
        }
    }
    property bool powerEnabled: false
    LazyLoader {
        id: powerLoader
        active: root.powerEnabled
        Power {
            sharedBackdrop: true
            handoff: root.handoffSurface === "power"
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
            sharedBackdrop: true
            handoff: root.handoffSurface === "clipboard"
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
            sharedBackdrop: true
            handoff: root.handoffSurface === "launcher"
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
        sourceComponent: niriAdapter
    }
    Component {
        id: niriAdapter
        NiriBackend.Adapter {}
    }
    // Only the composition root selects a backend. UI receives the interface.
    LazyLoader {
        active: root.probeEnabled
        Probe {
            compositor: root.compositorBackend
            lifecycle: root
        }
    }
    property string handoffSurface: ""
    ExternalSurface {
        id: monitorSurface
        toggleAction: "toggle-native"
        commandPrefix: [Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl", "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "monitor"]
        onFailed: message => console.warn("Monitor: " + message)
    }
    SurfaceRouter {
        id: surfaceRouter
        surfaces: ({
                monitor: {
                    independent: true,
                    external: true,
                    item: () => null,
                    alive: () => false,
                    open: () => monitorSurface.send("toggle"),
                    dismiss: () => monitorSurface.send("close")
                },
                launcher: {
                    item: () => launcherLoader.item,
                    alive: () => root.launcherEnabled,
                    open: handoff => {
                        root.handoffSurface = handoff ? "launcher" : "";
                        root.launcherEnabled = true;
                    },
                    dismiss: () => launcherLoader.item?.dismiss(),
                    present: () => launcherLoader.item?.present()
                },
                clipboard: {
                    item: () => clipboardLoader.item,
                    alive: () => root.clipboardEnabled,
                    open: handoff => {
                        root.handoffSurface = handoff ? "clipboard" : "";
                        root.clipboardEnabled = true;
                    },
                    dismiss: () => clipboardLoader.item?.dismiss(),
                    present: () => clipboardLoader.item?.present()
                },
                bluetooth: {
                    retiring: () => !!bluetoothLoader.item?.closeAfterControls,
                    item: () => bluetoothLoader.item,
                    alive: () => root.bluetoothEnabled,
                    open: handoff => {
                        root.handoffSurface = handoff ? "bluetooth" : "";
                        root.bluetoothEnabled = true;
                    },
                    dismiss: () => bluetoothLoader.item?.dismiss(),
                    present: () => bluetoothLoader.item?.present()
                },
                power: {
                    item: () => powerLoader.item,
                    alive: () => root.powerEnabled,
                    open: handoff => {
                        root.handoffSurface = handoff ? "power" : "";
                        root.powerEnabled = true;
                    },
                    dismiss: () => powerLoader.item?.dismiss(),
                    present: () => powerLoader.item?.present()
                },
                controlCenter: {
                    independent: true,
                    item: () => controlCenterLoader.item,
                    alive: () => root.controlCenterEnabled,
                    open: handoff => {
                        root.handoffSurface = handoff ? "controlCenter" : "";
                        root.controlCenterEnabled = true;
                    },
                    dismiss: () => controlCenterLoader.item?.dismiss(),
                    present: () => controlCenterLoader.item?.present()
                }
            })
    }
    IpcHandler {
        target: "foundation"
        function toggleMedia(): string {
            return mediaControl.toggle();
        }
        function toggleOtherMedia(): string {
            return mediaControl.toggle(true);
        }
        function nextMedia(): string {
            return mediaControl.skip(true);
        }
        function previousMedia(): string {
            return mediaControl.skip(false);
        }
        function reloadConfiguration(): string {
            if (powerLoader.item?.busy || root.controlCenter?.busy)
                return "busy";
            // Reply before replacing this IPC handler and its windows.
            Qt.callLater(() => Quickshell.reload(true));
            return "queued";
        }
        function reloadTheme(): void {
            Theme.reload();
            if (backdropLoader.item)
                backdropLoader.item.reload();
        }
        function showVolume(percent: string, muted: bool): void {
            volumeReadout.present(Number(percent), muted);
        }
        function showAudioNotice(message: string): void {
            volumeReadout.presentMessage(message);
        }
        function volumeStatus(): string {
            return JSON.stringify({
                visible: volumeReadout.visible,
                level: volumeReadout.level,
                muted: volumeReadout.muted
            });
        }
        function toggleClock(): void {
            clockVisibility.toggle();
        }
        function clockStatus(): string {
            return JSON.stringify({
                visible: root.clockVisible,
                recordingPhase: recordingState.phase,
                alive: clockLoader.item !== null
            });
        }
        function toggleControlCenter(style: string): void {
            Theme.refreshMotionPreference();
            root.controlCenterStyle = JSON.parse(style);
            surfaceRouter.request("controlCenter");
        }
        function toggleMonitor(): void {
            surfaceRouter.request("monitor");
        }
        function controlCenterStatus(): string {
            return JSON.stringify(root.controlCenter ? root.controlCenter.snapshot() : {
                alive: false,
                visible: false
            });
        }
        function toggleBluetooth(): void {
            Theme.refreshMotionPreference();
            surfaceRouter.request("bluetooth", false);
        }
        function bluetoothStatus(): string {
            return JSON.stringify(bluetoothLoader.item ? bluetoothLoader.item.snapshot() : {
                visible: false,
                alive: false
            });
        }
        function togglePower(): void {
            Theme.refreshMotionPreference();
            surfaceRouter.request("power", false);
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
            Theme.refreshMotionPreference();
            surfaceRouter.request("clipboard", false);
        }
        function showClipboard(): void {
            Theme.refreshMotionPreference();
            surfaceRouter.request("clipboard", true);
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
            Theme.refreshMotionPreference();
            surfaceRouter.request("launcher", false);
        }
        function showLauncher(): void {
            Theme.refreshMotionPreference();
            surfaceRouter.request("launcher", true);
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
