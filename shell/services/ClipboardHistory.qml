import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
    id: root
    // Keep external executables as filesystem paths; resolvedUrl blackholes paths
    // outside the Quickshell config directory. Override only for isolated tests.
    readonly property string workerOverride: Quickshell.env("DF_CLIPBOARD_WORKER")
    readonly property var backendCommand: workerOverride ? (workerOverride.endsWith(".py") ? ["python3", workerOverride] : [workerOverride]) : [Quickshell.shellDir + "/../native/foundation/target/release/desktop-foundationctl", "--root", Quickshell.shellDir + "/..", "clipboard"]
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
                const saved = JSON.parse(text());
                if (!Array.isArray(saved))
                    throw new Error("History index is not a list");
                root.entries = saved;
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
        stderr: StdioCollector {
            id: actionErrors
            onStreamFinished: {
                if (text.trim())
                    console.warn("Clipboard " + actionProcess.action + ": " + text.trim());
            }
        }
        // qmllint disable signal-handler-parameters
        // Installed qmltypes omit QProcess::ExitStatus; only exitCode is used.
        onExited: exitCode => {
            if (exitCode === 0) {
                root.error = "";
                root.completed(action);
            } else {
                root.error = root.failureMessage(action, actionErrors.text);
            }
        }
        // qmllint enable signal-handler-parameters
    }
    function failureMessage(action: string, message: string): string {
        const lines = message.split("\n").map(line => line.trim()).filter(line => line);
        const first = lines.find(line => line.startsWith("Foundation:")) || lines[0] || "";
        const detail = first.replace(/^Foundation:\s*/, "").replace(/[\u0000-\u001f\u007f]/g, " ").replace(/\s+/g, " ").trim().slice(0, 220);
        if (message.includes("history was not reset"))
            return "Clipboard history is unavailable. Saved items were kept.";
        if ((action === "pin" || action === "unpin") && detail.startsWith("Pinned history is full"))
            return detail;
        if ((action === "copy" || action === "copy-plain") && /file.*(unavailable|no longer available|missing)/i.test(detail))
            return detail.replace(/\.$/, "") + ". Restore the original, then try again.";
        if (action === "copy" || action === "copy-plain")
            return "Could not copy this item. Try again.";
        if (action === "pin" || action === "unpin")
            return "Could not change this pin. Try again.";
        if (action === "clear" || action === "clear-all")
            return "Could not clear clipboard history. Try again.";
        if (action === "init")
            return "Clipboard history could not be refreshed.";
        return "Could not " + action + " this history item. Try again.";
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
