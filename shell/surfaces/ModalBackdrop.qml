pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "../theme"
import "../components"

// One compositor blur surface for the whole centered-popup session.
// Top stays below Overlay widgets, so it cannot blur the incoming card itself.
// qmllint disable uncreatable-type
PanelWindow {
    id: root
    property var targetScreen: null
    property bool requested: false
    screen: targetScreen
    visible: requested || scrim.opacity > 0
    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-modal-backdrop"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    mask: Region {}
    color: "transparent"
    Rectangle {
        id: scrim
        objectName: "modal-backdrop-scrim"
        anchors.fill: parent
        color: Theme.colors.scrim
        opacity: root.requested ? 1 : 0
        Behavior on opacity {
            SurfaceAnimation {
                duration: root.requested ? Theme.timing.normal : Theme.timing.exit
            }
        }
    }
}
