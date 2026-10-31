import QtQuick
import Quickshell.Io

// Transport only. All device operations and validation live in Rust.
Process {
    id: root
    property var response: ({})
    property string failure: ""
    property bool expectJson: true
    property bool responseValid: false
    property string phase: "idle"
    signal finished(int code)
    onStarted: {
        response = ({});
        failure = "";
        responseValid = false;
        phase = "loading";
    }
    stdout: StdioCollector {
        onStreamFinished: {
            try {
                const value = JSON.parse(text);
                if (!value || typeof value !== "object" || Array.isArray(value))
                    throw new Error("Expected an object");
                root.response = value;
                root.responseValid = true;
            } catch (_) {
                if (root.expectJson)
                    root.failure = "Backend returned an invalid response.";
            }
        }
    }
    stderr: StdioCollector {
        onStreamFinished: {
            if (text.trim())
                root.failure = text.trim().slice(0, 4096);
        }
    }
    onExited: code => {
        const result = code === 0 && expectJson && !responseValid ? 65 : code;
        phase = result === 0 ? "ready" : "error";
        if (result !== 0 && !failure)
            failure = "The backend request failed.";
        finished(result);
    }
}
