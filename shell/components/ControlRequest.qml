import QtQuick
import Quickshell.Io

// Transport only. All device operations and validation live in Rust.
Process {
    id: root
    property var response: ({})
    property string failure: ""
    signal finished(int code)
    onStarted: {
        response = ({});
        failure = "";
    }
    stdout: StdioCollector {
        onStreamFinished: {
            try {
                root.response = JSON.parse(text);
            } catch (_) {}
        }
    }
    stderr: StdioCollector {
        onStreamFinished: root.failure = text.trim()
    }
    onExited: code => finished(code)
}
