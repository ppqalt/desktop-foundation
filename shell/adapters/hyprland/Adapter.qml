import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland

QtObject {
    id: root
    readonly property var waylandFocused: ToplevelManager.activeToplevel
    readonly property string backend: "hyprland"
    readonly property bool ready: Hyprland.focusedMonitor !== null
    readonly property var capabilities: ({
            moveWindow: true,
            moveWindowToOutput: true,
            moveWorkspaceToOutput: true,
            setFullscreen: true,
            setMaximized: true,
            setFloating: true,
            focusDirection: true,
            screenshotRegion: true,
            screenshotWindow: true,
            screenshotOutput: true,
            nativeOverview: false
        })
    property bool focusCleared: false
    property bool refreshPending: false
    property int eventCount: 0
    property double eventWindowStart: 0
    property int eventsInWindow: 0
    property bool debugEvents: Quickshell.env("DF_DEBUG_EVENTS") === "1"
    property var eventConnection: Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (root.debugEvents) {
                root.eventCount++;
                const now = Date.now();
                if (now - root.eventWindowStart > 1000) {
                    root.eventWindowStart = now;
                    root.eventsInWindow = 0;
                }
                if (++root.eventsInWindow === 1000)
                    console.warn("Compositor event storm: >=1000 events in one second");
            }
            if (event.name === "activewindowv2")
                root.focusCleared = !event.data;
            if (["fullscreen", "changefloatingmode", "movewindow", "movewindowv2", "activewindowv2", "resizeactive", "moveworkspace"].includes(event.name) && !root.refreshPending) {
                root.refreshPending = true;
                Qt.callLater(() => {
                    root.refreshPending = false;
                    Hyprland.refreshToplevels();
                });
            }
        }
    }
    readonly property var outputs: Hyprland.monitors.values.filter(m => m.scale > 0 && m.width > 0 && m.name !== "?").map(m => ({
                id: String(m.id),
                name: m.name,
                label: m.lastIpcObject.description ?? m.name,
                geometry: {
                    x: m.x,
                    y: m.y,
                    width: m.width / m.scale,
                    height: m.height / m.scale
                },
                scale: m.scale,
                focused: Hyprland.focusedWorkspace !== null && Hyprland.focusedWorkspace.monitor !== null && Hyprland.focusedWorkspace.monitor.id === m.id,
                workspaceId: m.activeWorkspace ? String(m.activeWorkspace.id) : null
            }))
    // Plain records form the UI contract; native objects remain in this adapter.
    readonly property var workspaces: Hyprland.workspaces.values.map(w => ({
                id: String(w.id),
                name: w.name,
                active: w.active,
                outputId: w.monitor ? String(w.monitor.id) : null,
                focused: w.focused
            }))
    readonly property var activeWorkspace: Hyprland.focusedWorkspace ? ({
            id: String(Hyprland.focusedWorkspace.id),
            name: Hyprland.focusedWorkspace.name
        }) : null
    readonly property var windows: Hyprland.toplevels.values.map(w => ({
                id: w.address.startsWith("0x") ? w.address : "0x" + w.address,
                title: w.title,
                appId: (w.wayland ? w.wayland.appId : (w.lastIpcObject.class ?? "")),
                focused: !root.focusCleared && w.workspace === Hyprland.focusedWorkspace && (Hyprland.activeToplevel === w || (root.waylandFocused !== null && w.wayland === root.waylandFocused)),
                outputId: w.monitor ? String(w.monitor.id) : null,
                geometry: {
                    x: w.lastIpcObject.at?.[0] ?? null,
                    y: w.lastIpcObject.at?.[1] ?? null,
                    width: w.lastIpcObject.size?.[0] ?? null,
                    height: w.lastIpcObject.size?.[1] ?? null
                },
                state: {
                    fullscreen: w.lastIpcObject.fullscreen === 2,
                    maximized: w.lastIpcObject.fullscreen === 1,
                    floating: w.lastIpcObject.floating ?? null,
                    urgent: w.urgent
                },
                workspaceId: w.workspace ? String(w.workspace.id) : null
            }))
    readonly property var focusedWindow: windows.find(w => w.focused) ?? null
    function windowSelector(id: string): string {
        if (!/^0x[0-9a-fA-F]+$/.test(id))
            throw new Error("Invalid window ID");
        return '"address:' + id + '"';
    }
    function workspaceSelector(id: string): string {
        if (!/^-?\d+$/.test(id))
            throw new Error("Invalid workspace ID");
        return id;
    }
    function focusWindow(id: string): void {
        Hyprland.dispatch("hl.dsp.focus({window=" + windowSelector(id) + "})");
    }
    function closeWindow(id: string): void {
        Hyprland.dispatch("hl.dsp.window.close({window=" + windowSelector(id) + "})");
    }
    function focusWorkspace(id: string): void {
        Hyprland.dispatch("hl.dsp.focus({workspace=" + workspaceSelector(id) + "})");
    }
    function moveWindow(id: string, workspaceId: string): void {
        Hyprland.dispatch("hl.dsp.window.move({window=" + windowSelector(id) + ",workspace=" + workspaceSelector(workspaceId) + ",follow=false})");
    }
    function setFullscreen(id: string, enabled: bool): void {
        Hyprland.dispatch("hl.dsp.window.fullscreen({window=" + windowSelector(id) + ",mode=\"fullscreen\",action=\"" + (enabled ? "set" : "unset") + "\"})");
    }
    function setMaximized(id: string, enabled: bool): void {
        Hyprland.dispatch("hl.dsp.window.fullscreen({window=" + windowSelector(id) + ",mode=\"maximized\",action=\"" + (enabled ? "set" : "unset") + "\"})");
    }
    function setFloating(id: string, enabled: bool): void {
        Hyprland.dispatch("hl.dsp.window.float({window=" + windowSelector(id) + ",action=\"" + (enabled ? "on" : "off") + "\"})");
    }
    function focusDirection(direction: string): void {
        if (!["left", "right", "up", "down"].includes(direction))
            throw new Error("Invalid direction");
        Hyprland.dispatch('hl.dsp.focus({direction="' + direction + '"})');
    }
    function outputSelector(id: string): string {
        if (!/^\d+$/.test(id))
            throw new Error("Invalid output ID");
        return id;
    }
    function moveWindowToOutput(id: string, outputId: string): void {
        Hyprland.dispatch("hl.dsp.window.move({window=" + windowSelector(id) + ",monitor=" + outputSelector(outputId) + ",follow=false})");
    }
    function moveWorkspaceToOutput(id: string, outputId: string): void {
        Hyprland.dispatch("hl.dsp.workspace.move({workspace=" + workspaceSelector(id) + ",monitor=" + outputSelector(outputId) + "})");
    }
    function scrollColumns(direction: string): void {
        if (!["left", "right"].includes(direction))
            throw new Error("Invalid column direction");
        Hyprland.dispatch('hl.dsp.focus({direction="' + direction + '"})');
    }
    function screenshotRegion(): void {
        Quickshell.execDetached([Quickshell.shellDir + "/../scripts/screenshot", "--backend", "hyprland", "region"]);
    }
    function screenshotWindow(): void {
        Quickshell.execDetached([Quickshell.shellDir + "/../scripts/screenshot", "--backend", "hyprland", "window"]);
    }
    function screenshotOutput(): void {
        Quickshell.execDetached([Quickshell.shellDir + "/../scripts/screenshot", "--backend", "hyprland", "output"]);
    }
    function toggleOverview(): bool {
        return false;
    }
}
