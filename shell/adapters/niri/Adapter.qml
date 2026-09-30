import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
    id: root
    readonly property string backend: "niri"
    property var rawWindows: []
    property var rawWorkspaces: []
    property var rawOutputs: ({})
    property bool haveWindows: false
    property bool haveWorkspaces: false
    property bool haveOutputs: false
    readonly property bool ready: stream.connected && haveWindows && haveWorkspaces && haveOutputs
    property int eventCount: 0
    property string lastError: ""
    property bool overviewOpen: false
    property bool configFailed: false
    property var keyboardLayouts: ({})
    property var pending: []
    property var currentRequest: null
    property bool outputRefreshPending: false
    property int retryDelay: 1000
    readonly property var capabilities: ({
            moveWindow: true,
            moveWindowToOutput: true,
            moveWorkspaceToOutput: true,
            setFullscreen: false,
            toggleFullscreen: true,
            setMaximized: false,
            maximizeColumn: true,
            setFloating: true,
            focusDirection: true,
            moveDirection: true,
            toggleFloating: true,
            nativeOverview: true,
            screenshotRegion: true,
            screenshotWindow: true,
            screenshotOutput: true,
            cycleColumnWidth: true,
            centerColumn: true
        })
    readonly property var activeWorkspace: rawWorkspaces.find(w => w.is_focused) ? ({
            id: String(rawWorkspaces.find(w => w.is_focused).id),
            name: rawWorkspaces.find(w => w.is_focused).name ?? String(rawWorkspaces.find(w => w.is_focused).idx)
        }) : null
    readonly property var focusedWorkspace: activeWorkspace
    readonly property var outputs: Object.values(rawOutputs).filter(o => o.logical).map(o => ({
                id: o.name,
                name: o.name,
                label: [o.make, o.model].filter(Boolean).join(" "),
                geometry: {
                    x: o.logical.x,
                    y: o.logical.y,
                    width: o.logical.width,
                    height: o.logical.height
                },
                scale: o.logical.scale,
                focused: rawWorkspaces.some(w => w.output === o.name && w.is_focused),
                workspaceId: String(rawWorkspaces.find(w => w.output === o.name && w.is_active)?.id ?? "")
            }))
    readonly property var workspaces: rawWorkspaces.map(w => ({
                id: String(w.id),
                name: w.name ?? String(w.idx),
                index: w.idx,
                outputId: w.output ?? null,
                active: w.is_active,
                focused: w.is_focused,
                urgent: w.is_urgent
            }))
    readonly property var windows: rawWindows.map(w => ({
                id: String(w.id),
                title: w.title ?? "",
                appId: w.app_id ?? "",
                focused: w.is_focused,
                outputId: rawWorkspaces.find(s => s.id === w.workspace_id)?.output ?? null,
                workspaceId: w.workspace_id === null ? null : String(w.workspace_id),
                geometry: {
                    x: w.layout?.tile_pos_in_workspace_view?.[0] ?? null,
                    y: w.layout?.tile_pos_in_workspace_view?.[1] ?? null,
                    width: w.layout?.window_size?.[0] ?? null,
                    height: w.layout?.window_size?.[1] ?? null
                },
                state: {
                    fullscreen: null,
                    maximized: null,
                    floating: w.is_floating,
                    urgent: w.is_urgent
                },
                niri: {
                    layout: w.layout,
                    focusTimestamp: w.focus_timestamp
                }
            }))
    readonly property var focusedWindow: windows.find(w => w.focused) ?? null
    property var stream: Socket {
        id: stream
        path: Quickshell.env("NIRI_SOCKET")
        connected: true
        onConnectedChanged: {
            if (connected) {
                root.retryDelay = 1000;
                write(JSON.stringify("EventStream") + "\n");
                flush();
                root.refreshOutputs();
            } else {
                root.rawWindows = [];
                root.rawWorkspaces = [];
                root.rawOutputs = ({});
                root.haveWindows = false;
                root.haveWorkspaces = false;
                root.haveOutputs = false;
                root.pending = [];
                root.currentRequest = null;
                requests.connected = false;
                retry.start();
            }
        }
        // qmllint disable signal-handler-parameters
        onError: {
            root.lastError = "Niri event socket failed";
            connected = false;
            retry.start();
        }
        // qmllint enable signal-handler-parameters
        parser: SplitParser {
            splitMarker: "\n"
            onRead: data => root.consume(data)
        }
    }
    // Only disconnection triggers retry; no periodic state refresh.
    property var retry: Timer {
        id: retry
        interval: root.retryDelay
        repeat: false
        onTriggered: {
            root.retryDelay = Math.min(30000, root.retryDelay * 2);
            stream.connected = true;
        }
    }
    property var requests: Socket {
        id: requests
        path: Quickshell.env("NIRI_SOCKET")
        onConnectedChanged: {
            if (connected && root.currentRequest) {
                write(JSON.stringify(root.currentRequest) + "\n");
                flush();
            }
        }
        // qmllint disable signal-handler-parameters
        // Installed metadata omits QLocalSocket::LocalSocketError.
        onError: {
            root.lastError = "Niri request socket failed";
            connected = false;
            root.currentRequest = null;
            root.pending = [];
        }
        // qmllint enable signal-handler-parameters
        parser: SplitParser {
            splitMarker: "\n"
            onRead: data => {
                try {
                    const reply = JSON.parse(data);
                    if (reply.Err)
                        root.lastError = String(reply.Err);
                    else {
                        root.lastError = "";
                        if (reply.Ok?.Outputs) {
                            root.rawOutputs = reply.Ok.Outputs;
                            root.haveOutputs = true;
                        }
                    }
                } catch (e) {
                    root.lastError = "Invalid IPC reply";
                }
                requests.connected = false;
                root.currentRequest = null;
                Qt.callLater(root.nextRequest);
            }
        }
    }
    function send(request): void {
        if (!stream.connected)
            return;
        if (pending.length >= 64) {
            lastError = "IPC request queue is full";
            return;
        }
        pending = pending.concat([request]);
        nextRequest();
    }
    function nextRequest(): void {
        if (currentRequest || !pending.length || !stream.connected)
            return;
        currentRequest = pending[0];
        pending = pending.slice(1);
        requests.connected = true;
    }
    function refreshOutputs(): void {
        if (outputRefreshPending)
            return;
        outputRefreshPending = true;
        Qt.callLater(() => {
            root.outputRefreshPending = false;
            root.send("Outputs");
        });
    }
    function consume(data: string): void {
        let event;
        try {
            event = JSON.parse(data);
        } catch (e) {
            lastError = "Invalid event stream record";
            return;
        }
        if (event.Ok)
            return;
        eventCount++;
        if (event.WorkspacesChanged) {
            rawWorkspaces = event.WorkspacesChanged.workspaces;
            haveWorkspaces = true;
            refreshOutputs();
        } else if (event.WorkspaceUrgencyChanged)
            rawWorkspaces = rawWorkspaces.map(w => w.id === event.WorkspaceUrgencyChanged.id ? Object.assign({}, w, {
                    is_urgent: event.WorkspaceUrgencyChanged.urgent
                }) : w);
        else if (event.WindowsChanged) {
            rawWindows = event.WindowsChanged.windows;
            haveWindows = true;
        } else if (event.WindowOpenedOrChanged) {
            const w = event.WindowOpenedOrChanged.window;
            rawWindows = rawWindows.filter(old => old.id !== w.id).map(old => w.is_focused ? Object.assign({}, old, {
                    is_focused: false
                }) : old).concat([w]);
        } else if (event.WindowClosed)
            rawWindows = rawWindows.filter(w => w.id !== event.WindowClosed.id);
        else if (event.WindowFocusChanged)
            rawWindows = rawWindows.map(w => Object.assign({}, w, {
                    is_focused: w.id === event.WindowFocusChanged.id
                }));
        else if (event.WorkspaceActivated) {
            const value = event.WorkspaceActivated;
            const output = rawWorkspaces.find(w => w.id === value.id)?.output;
            rawWorkspaces = rawWorkspaces.map(w => Object.assign({}, w, {
                    is_active: w.output === output ? w.id === value.id : w.is_active,
                    is_focused: value.focused ? w.id === value.id : w.is_focused
                }));
        } else if (event.WorkspaceActiveWindowChanged)
            rawWorkspaces = rawWorkspaces.map(w => w.id === event.WorkspaceActiveWindowChanged.workspace_id ? Object.assign({}, w, {
                    active_window_id: event.WorkspaceActiveWindowChanged.active_window_id
                }) : w);
        else if (event.WindowLayoutsChanged)
            rawWindows = rawWindows.map(w => {
                const change = event.WindowLayoutsChanged.changes.find(pair => pair[0] === w.id);
                return change ? Object.assign({}, w, {
                    layout: change[1]
                }) : w;
            });
        else if (event.WindowUrgencyChanged)
            rawWindows = rawWindows.map(w => w.id === event.WindowUrgencyChanged.id ? Object.assign({}, w, {
                    is_urgent: event.WindowUrgencyChanged.urgent
                }) : w);
        else if (event.KeyboardLayoutsChanged)
            keyboardLayouts = event.KeyboardLayoutsChanged.keyboard_layouts;
        else if (event.KeyboardLayoutSwitched)
            keyboardLayouts = Object.assign({}, keyboardLayouts, {
                current_idx: event.KeyboardLayoutSwitched.idx
            });
        else if (event.OverviewOpenedOrClosed)
            overviewOpen = event.OverviewOpenedOrClosed.is_open;
        else if (event.ConfigLoaded) {
            configFailed = event.ConfigLoaded.failed;
            refreshOutputs();
        }
        // Unknown additive events intentionally do not reset the model.
    }
    function identity(id: string): double {
        if (!/^\d+$/.test(id) || !Number.isSafeInteger(Number(id)))
            throw new Error("Invalid Niri ID");
        return Number(id);
    }
    function action(name: string, args): void {
        let value = ({});
        value[name] = args ?? ({});
        send({
            Action: value
        });
    }
    function focusWindow(id: string): void {
        action("FocusWindow", {
            id: identity(id)
        });
    }
    function closeWindow(id: string): void {
        action("CloseWindow", {
            id: identity(id)
        });
    }
    function focusWorkspace(id: string): void {
        action("FocusWorkspace", {
            reference: {
                Id: identity(id)
            }
        });
    }
    function moveWindow(id: string, workspaceId: string): void {
        action("MoveWindowToWorkspace", {
            window_id: identity(id),
            reference: {
                Id: identity(workspaceId)
            },
            focus: false
        });
    }
    function setFloating(id: string, enabled: bool): void {
        action(enabled ? "MoveWindowToFloating" : "MoveWindowToTiling", {
            id: identity(id)
        });
    }
    function toggleFullscreen(id: string): void {
        action("FullscreenWindow", {
            id: identity(id)
        });
    }
    function setFullscreen(_id: string, _enabled: bool): bool {
        return false;
    }
    function setMaximized(_id: string, _enabled: bool): bool {
        return false;
    }
    function maximizeColumn(): void {
        action("MaximizeColumn", {});
    }
    function cycleColumnWidth(): void {
        action("SwitchPresetColumnWidth", {});
    }
    function centerColumn(): void {
        action("CenterColumn", {});
    }
    function toggleFloating(id: string): void {
        action("ToggleWindowFloating", {
            id: identity(id)
        });
    }
    function focusDirection(direction: string): void {
        const map = {
            left: "FocusColumnLeft",
            right: "FocusColumnRight",
            up: "FocusWindowUp",
            down: "FocusWindowDown"
        };
        if (!map[direction])
            throw new Error("Invalid direction");
        action(map[direction], {});
    }
    function moveDirection(direction: string): void {
        const map = {
            left: "MoveColumnLeft",
            right: "MoveColumnRight",
            up: "MoveWindowUp",
            down: "MoveWindowDown"
        };
        if (!map[direction])
            throw new Error("Invalid direction");
        action(map[direction], {});
    }
    function scrollColumns(direction: string): void {
        focusDirection(direction);
    }
    function moveWindowToOutput(id: string, outputId: string): void {
        action("MoveWindowToMonitor", {
            id: identity(id),
            output: outputId
        });
    }
    function moveWorkspaceToOutput(id: string, outputId: string): void {
        action("MoveWorkspaceToMonitor", {
            reference: {
                Id: identity(id)
            },
            output: outputId
        });
    }
    function toggleOverview(): bool {
        action("ToggleOverview", {});
        return true;
    }
    function screenshotRegion(): void {
        Quickshell.execDetached([Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/screenshot", "--backend", "niri", "region"]);
    }
    function screenshotWindow(): void {
        action("ScreenshotWindow", {
            id: null,
            write_to_disk: false,
            show_pointer: false
        });
    }
    function screenshotOutput(): void {
        action("ScreenshotScreen", {
            write_to_disk: false,
            show_pointer: false
        });
    }
}
