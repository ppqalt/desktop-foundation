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
    readonly property bool visible: ["starting", "recording", "stopping", "error"].includes(phase)
    property bool prepared: false
    property ControlRequest initialize: ControlRequest {
        command: [Quickshell.shellDir + "/../scripts/record", "status"]
        running: true
        onFinished: code => {
            if (code === 0) {
                root.state = response;
                root.prepared = true;
            }
        }
    }
    property FileView statusFile: FileView {
        path: root.prepared ? (Quickshell.env("XDG_RUNTIME_DIR") || "") + "/desktop-foundation-recording/status.json" : ""
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                const value = JSON.parse(text());
                root.state = ["idle", "starting", "recording", "stopping", "error"].includes(value.phase) ? value : {
                    phase: "error"
                };
            } catch (e) {
                root.state = {
                    phase: "error"
                };
            }
        }
    }
}
