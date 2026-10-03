import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
    id: root
    // Keep external executables as filesystem paths; resolvedUrl blackholes paths
    // outside the Quickshell config directory. Override only for isolated tests.
    readonly property string workerOverride: Quickshell.env("DF_CLIPBOARD_WORKER")
    readonly property var backendCommand: workerOverride ? (workerOverride.endsWith(".py") ? ["python3", workerOverride] : [workerOverride]) : [Quickshell.shellDir + "/../scripts/foundation", "clipboard"]
    readonly property string stateDirectory: Quickshell.env("DF_CLIPBOARD_STATE") || (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/desktop-foundation/clipboard"
    property var entries: []
    property bool loading: true
    property int startupRetries: 0
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
            root.error = root.startupRetries < 20 ? "Clipboard history is starting…" : "Clipboard history is unavailable.";
            root.loading = root.startupRetries < 20;
        }
    }
    // Startup may overlap database initialization. Retry only while missing,
    // for at most three seconds; file watching handles subsequent updates.
    property Timer startupRetry: Timer {
        interval: 150
        running: root.loading && root.startupRetries < 20
        repeat: true
        onTriggered: {
            root.startupRetries++;
            root.indexFile.reload();
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
        actionProcess.command = root.backendCommand.concat(["--state", root.stateDirectory, action]).concat(identity ? [identity] : []);
        actionProcess.running = true;
    }
}
