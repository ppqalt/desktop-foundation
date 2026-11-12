import QtQuick
import Quickshell
import Quickshell.Io
import "../components"

// Confirmed display snapshots survive lazy panels. Rust owns disk validation.
QtObject {
    id: root
    property var snapshot: ({})
    property int revision: 0
    property bool requested: false
    property string pending: ""
    property string inFlight: ""
    property string saved: ""
    property string failed: ""
    property string error: ""
    readonly property bool busy: reader.running || writer.running || pending !== ""
    property string backend: Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl"
    function restore(): void {
        if (requested || Object.keys(snapshot).length)
            return;
        requested = true;
        reader.revision = revision;
        reader.running = true;
    }
    function remember(value): void {
        const next = Object.assign({}, snapshot);
        let usable = false;
        for (const key of ["audio", "network"]) {
            if (value[key]?.available === true) {
                next[key] = value[key];
                usable = true;
            }
        }
        if (!usable)
            return;
        revision++;
        snapshot = next;
        pending = JSON.stringify(next);
        pump();
    }
    function pump(): void {
        if (writer.running || !pending)
            return;
        const payload = pending;
        pending = "";
        if (payload === saved || payload === failed)
            return;
        inFlight = payload;
        writer.stdinEnabled = true;
        writer.running = true;
    }
    property ControlRequest reader: ControlRequest {
        property int revision: 0
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "snapshot-cache", "read"]
        onFinished: code => {
            if (code === 0 && response.snapshot && typeof response.snapshot === "object" && !Array.isArray(response.snapshot)) {
                const next = Object.assign({}, root.snapshot);
                let added = false;
                for (const key of ["audio", "network"]) {
                    if (!next[key] && response.snapshot[key]?.available === true) {
                        next[key] = response.snapshot[key];
                        added = true;
                    }
                }
                if (revision === root.revision) {
                    root.snapshot = next;
                    root.saved = JSON.stringify(next);
                } else if (added) {
                    // A fresh audio result must not discard the last network
                    // observation merely because the cache read finished later.
                    root.snapshot = next;
                    root.pending = JSON.stringify(next);
                    root.pump();
                }
            }
        }
    }
    property Process writer: Process {
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "snapshot-cache", "write"]
        onStarted: {
            write(root.inFlight);
            stdinEnabled = false;
        }
        stdout: StdioCollector {}
        stderr: StdioCollector {
            onStreamFinished: root.error = text.trim().slice(0, 4096)
        }
        onExited: code => {
            if (code === 0) {
                root.saved = root.inFlight;
                root.failed = "";
                root.error = "";
            } else {
                root.failed = root.inFlight;
                console.warn("Could not remember Control Center state:", root.error);
            }
            Qt.callLater(root.pump);
        }
    }
}
