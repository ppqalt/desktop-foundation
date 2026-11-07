pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "../components"

// The Rust owner publishes confirmed state; no timer or process polling.
QtObject {
    id: root
    property var state: ({
            phase: "idle"
        })
    readonly property string phase: state.phase ?? "idle"
    readonly property bool replayActive: (state.replay === true && ["starting", "stopping"].includes(phase)) || ["replay", "replay-recording"].includes(phase)
    readonly property bool visible: ["starting", "recording", "stopping", "error", "replay", "replay-recording"].includes(phase)
    property bool prepared: false
    property ControlRequest initialize: ControlRequest {
        command: [Quickshell.shellDir + "/../scripts/record", "status"]
        running: true
        onFinished: code => {
            root.state = code === 0 ? response : {
                phase: "error",
                error: failure
            };
            root.prepared = true;
        }
    }
    property FileView statusFile: FileView {
        path: root.prepared ? (Quickshell.env("XDG_RUNTIME_DIR") || "") + "/desktop-foundation-recording/status.json" : ""
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            if (!root.prepared)
                return;
            try {
                const value = JSON.parse(text());
                root.state = ["idle", "starting", "recording", "stopping", "error", "replay", "replay-recording"].includes(value.phase) ? value : {
                    phase: "error"
                };
            } catch (e) {
                root.state = {
                    phase: "error"
                };
            }
        }
        onLoadFailed: {
            if (root.prepared)
                root.state = {
                    phase: "error",
                    error: "Recording status is unavailable"
                };
        }
    }
}
