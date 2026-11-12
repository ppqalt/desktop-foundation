import QtQuick
import QtQuick.Window
import "../theme"

// Shared page drawing/input adapter, used unchanged in Control Center and gallery.
FocusScope {
    id: root
    default property alias contentData: canvas.data
    readonly property string currentKey: motion.currentKey
    readonly property string requestedKey: motion.requestedKey
    readonly property string phase: motion.phase
    readonly property bool running: motion.running
    readonly property bool acceptsInput: motion.acceptsInput
    property var previousFocus: null
    property int previousReason: Qt.OtherFocusReason
    signal committed(string key)
    signal settled
    function request(key, forward = true): void {
        if (key !== currentKey && acceptsInput) {
            previousFocus = root.Window.window?.activeFocusItem ?? null;
            previousReason = previousFocus?.focusReason ?? Qt.OtherFocusReason;
        }
        motion.request(key, forward);
    }
    clip: true
    // During exit, focus belongs to the pane. Escape still bubbles to its owner.
    Keys.onPressed: event => {
        if (!acceptsInput && event.key !== Qt.Key_Escape)
            event.accepted = true;
    }
    ContentTransition {
        id: motion
        onPhaseChanged: {
            if (!acceptsInput)
                root.forceActiveFocus(Qt.OtherFocusReason);
        }
        onCommitted: key => root.committed(key)
        onCancelled: {
            if (root.previousFocus?.enabled && root.previousFocus.visible)
                Material.restoreFocus(root.previousFocus, root.previousReason);
        }
        onSettled: root.settled()
    }
    Item {
        id: canvas
        anchors.fill: parent
        opacity: motion.opacity
        transform: Translate {
            x: motion.offset
        }
    }
    // Preserve enabled styling and the initiating press spring during exit.
    MouseArea {
        anchors.fill: parent
        enabled: !motion.acceptsInput
        acceptedButtons: Qt.AllButtons
        onWheel: event => event.accepted = true
    }
}
