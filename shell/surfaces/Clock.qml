pragma ComponentBehavior: Bound
import QtQuick
import "../components"
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
        radius: height / 2
        color: Material.colors.surface_container_low
        border.width: 1
        border.color: Qt.alpha(Material.colors.primary, 0.65)
        SurfaceOutline {
            cornerRadius: parent.radius
        }
        opacity: root.entered ? 1 : 0
        Behavior on opacity {
            SurfaceAnimation {
                duration: Theme.timing.fast
                easing.type: Theme.easing
            }
        }
        Text {
            anchors.centerIn: parent
            text: Qt.formatDateTime(clock.date, "hh:mm")
            color: Material.colors.on_surface
            font.family: Theme.typography.family
            font.pixelSize: Material.type.labelLarge
        }
    }
}
