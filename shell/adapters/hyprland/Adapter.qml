import QtQuick
import Quickshell.Hyprland

QtObject {
    id: root
    readonly property string backend: "hyprland"
    readonly property bool ready: Hyprland.focusedMonitor !== null
    readonly property var capabilities: ({ moveWindow: true, nativeOverview: false })
    // Plain records form the UI contract; native objects remain in this adapter.
    readonly property var workspaces: Hyprland.workspaces.values.map(w => ({
        id: String(w.id), name: w.name, active: w.active, focused: w.focused
    }))
    readonly property var activeWorkspace: Hyprland.focusedWorkspace ? ({
        id: String(Hyprland.focusedWorkspace.id), name: Hyprland.focusedWorkspace.name
    }) : null
    readonly property var windows: Hyprland.toplevels.values.map(w => ({
        id: w.HyprlandToplevel.address, title: w.title, appId: w.appId,
        focused: w.activated, workspaceId: w.HyprlandToplevel.workspace ? String(w.HyprlandToplevel.workspace.id) : null
    }))
    readonly property var focusedWindow: windows.find(w => w.focused) ?? null
    function windowSelector(id: string): string {
        if (!/^0x[0-9a-fA-F]+$/.test(id)) throw new Error("Invalid window ID");
        return '"address:' + id + '"';
    }
    function workspaceSelector(id: string): string {
        if (!/^-?\d+$/.test(id)) throw new Error("Invalid workspace ID");
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
    function toggleOverview(): bool { return false; }
}
