import QtQuick
import QtQuick.Window
import "../theme"

// Presentation/disposal only. Router intent and backend state have separate owners.
QtObject {
    id: root
    property Item targetItem: null
    property bool closeBlocked: false
    property bool canDispose: true
    property bool entered: false
    property bool closing: false
    property bool hasPresented: false
    property bool exitElapsed: false
    property bool finishedSent: false
    readonly property string phase: finishedSent ? "disposed" : closing ? "closing" : entered ? "open" : "opening"
    signal finished
    function framePresented(): void {
        if (!closing && !finishedSent) {
            hasPresented = true;
            entered = true;
        }
    }
    function present(): void {
        closeClock.stop();
        exitElapsed = false;
        finishedSent = false;
        closing = false;
        entered = hasPresented;
    }
    function dismiss(): bool {
        if (closeBlocked || closing || finishedSent)
            return false;
        closing = true;
        exitElapsed = false;
        closeClock.restart();
        return true;
    }
    function finishIfReady(): void {
        if (closing && exitElapsed && canDispose && !finishedSent) {
            finishedSent = true;
            finished();
        }
    }
    onCanDisposeChanged: finishIfReady()
    property Connections presentation: Connections {
        target: root.targetItem ? root.targetItem.Window.window : null
        enabled: !root.entered && !root.closing && !root.finishedSent
        function onFrameSwapped(): void {
            root.framePresented();
        }
    }
    property Timer closeClock: Timer {
        interval: Theme.timing.exit
        repeat: false
        onTriggered: {
            root.exitElapsed = true;
            root.finishIfReady();
        }
    }
}
