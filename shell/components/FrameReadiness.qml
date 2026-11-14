import QtQuick
import QtQuick.Window

// One observation per mapped surface instance, using the actual Qt swap signal.
// Submission is not presentation feedback, nor proof that the animation finished.
Item {
    id: root
    required property string surface
    property bool reported: false
    visible: false
    width: 0
    height: 0
    Connections {
        target: root.Window.window
        enabled: !root.reported && target !== null
        function onFrameSwapped(): void {
            root.reported = true;
            console.info("DF_FRAME_SUBMITTED surface=" + root.surface);
        }
    }
}
