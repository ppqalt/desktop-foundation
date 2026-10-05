pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "../theme"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    property var targetScreen: null
    property bool entered: false
    screen: targetScreen
    visible: true
    implicitWidth: 76
    implicitHeight: 34
    anchors {
        left: true
        top: true
    }
    margins.left: 24
    margins.top: 24
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-clock"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    color: "transparent"
    mask: Region {}
    Component.onCompleted: entered = true

    // The owner destroys this surface while hidden, including its minute timer.
    SystemClock {
        id: clock
        precision: SystemClock.Minutes
        enabled: root.visible
    }
    Rectangle {
        anchors.fill: parent
        radius: Theme.radii.small
        color: Theme.colors.background
        border.width: 1
        border.color: Theme.colors.border
        opacity: root.entered ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.timing.fast
                easing.type: Theme.easing
            }
        }
        Text {
            anchors.centerIn: parent
            text: Qt.formatDateTime(clock.date, "hh:mm")
            color: Theme.colors.foreground
            font.family: Theme.typography.mono
            font.pixelSize: 13
        }
    }
}
