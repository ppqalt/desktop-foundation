import QtQuick
import Quickshell
import Quickshell.Io
import "shell/surfaces"

// Private compositor fixture: no desktop services or hardware actions.
ShellRoot {
    Component.onCompleted: Quickshell.watchFiles = false
    ReloadNotice {}
    IpcHandler {
        target: "visual-review"
        function reload(): string {
            Qt.callLater(() => Quickshell.reload(true));
            return "queued";
        }
    }
    Timer {
        interval: 60000
        running: true
        onTriggered: Qt.quit()
    }
}
