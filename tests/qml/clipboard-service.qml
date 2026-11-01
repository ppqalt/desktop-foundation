import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Io
import "services"
ShellRoot {
    id: fixture
    property int stage: 0
    property int failures: 0
    function check(ok, detail) { if (!ok) { failures++; console.error("CLIPBOARD_SERVICE_FAIL " + detail); } }
    Window { visible: true; width: 64; height: 64 }
    ClipboardHistory { id: history }
    Process { id: corrupt; command: ["/bin/sh", "-c", "printf '{}' > \"$1\"", "fixture", Quickshell.env("DF_CLIPBOARD_STATE") + "/index.json"] }
    Timer {
        interval: 50; running: true; repeat: true
        onTriggered: {
            if (fixture.stage === 0 && !history.loading && history.entries.length === 1) {
                fixture.check(history.backendCommand.length === 4 && history.backendCommand[3] === "clipboard", "default command is not native");
                history.request("clear", "");
                fixture.stage = 1;
            } else if (fixture.stage === 1 && !history.busy && history.entries.length === 0) {
                fixture.check(!history.error, "clear failed");
                history.request("delete", "../escape");
                fixture.stage = 2;
            } else if (fixture.stage === 2 && !history.busy && history.error.indexOf("Could not delete") >= 0) {
                fixture.check(history.actionProcess.failure.indexOf("Invalid history ID") >= 0, "invalid history identity was not reported");
                corrupt.running = true;
                fixture.stage = 3;
            } else if (fixture.stage === 3 && !corrupt.running && history.error === "History could not be read.") {
                console.log("CLIPBOARD_SERVICE_RESULT " + JSON.stringify({failures: fixture.failures}));
                Qt.quit();
            }
        }
    }
    Timer { interval: 8000; running: true; onTriggered: { console.error("CLIPBOARD_SERVICE_FAIL timeout stage " + fixture.stage + " error " + history.error); Qt.exit(1); } }
}
