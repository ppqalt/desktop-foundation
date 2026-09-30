import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
    id: root
    // Keep external executables as filesystem paths; resolvedUrl blackholes paths
    // outside the Quickshell config directory. Override only for isolated tests.
    readonly property string backendScript: Quickshell.env("DF_CLIPBOARD_WORKER") || Quickshell.shellDir + "/../scripts/clipboard.py"
    readonly property string stateDirectory: Quickshell.env("DF_CLIPBOARD_STATE") || (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/desktop-foundation/clipboard"
    property var entries: []
    property bool loading: true
    property string error: ""
    readonly property bool busy: actionProcess.running
    signal completed(string action)
    property var indexFile: FileView {
        path: root.stateDirectory + "/index.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                root.entries = JSON.parse(text());
                root.loading = false;
                root.error = "";
            } catch (e) {
                root.error = "History could not be read.";
                root.loading = false;
            }
        }
        onLoadFailed: {
            root.error = "Clipboard history is unavailable.";
            root.loading = false;
        }
    }
    property var actionProcess: Process {
        id: actionProcess
        property string action: ""
        stderr: StdioCollector {}
        // qmllint disable signal-handler-parameters
        // Installed qmltypes omit QProcess::ExitStatus; only exitCode is used.
        onExited: exitCode => {
            if (exitCode === 0) {
                root.error = "";
                root.completed(action);
            } else
                root.error = "Could not " + action + " this history item. Try again.";
        }
        // qmllint enable signal-handler-parameters
    }
    function request(action: string, identity: string): void {
        if (actionProcess.running)
            return;
        root.error = "";
        actionProcess.action = action;
        actionProcess.command = ["python3", root.backendScript, "--state", root.stateDirectory, action].concat(identity ? [identity] : []);
        actionProcess.running = true;
    }
}
