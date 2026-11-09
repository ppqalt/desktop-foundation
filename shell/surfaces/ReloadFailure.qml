pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import "../theme"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    property string message: ""
    property var targetScreen: Quickshell.screens[0] ?? null
    screen: root.targetScreen
    visible: true
    implicitWidth: Math.min(360, Math.max(0, (root.targetScreen?.width ?? 392) - 32))
    implicitHeight: 76
    anchors.bottom: true
    anchors.right: true
    margins {
        bottom: 16
        right: 16
    }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation:reload-notice"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    mask: Region {}
    color: "transparent"
    Rectangle {
        anchors.fill: parent
        radius: Material.shape.small
        color: "#000000"
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 4
            Text {
                Layout.fillWidth: true
                text: "Shell reload failed"
                color: "#ffffff"
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
                Accessible.name: text + ": " + root.message
            }
            Text {
                Layout.fillWidth: true
                text: "Previous desktop kept · see shell logs"
                color: "#cccccc"
                font.family: Theme.typography.family
                font.pixelSize: Material.type.labelSmall
                elide: Text.ElideRight
            }
        }
    }
}
