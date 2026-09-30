import QtQuick
import "../theme"

Rectangle {
    id: root
    required property string label
    implicitWidth: text.implicitWidth + 12
    implicitHeight: 22
    radius: 5
    color: "transparent"
    border.color: Theme.colors.border
    Text {
        id: text
        anchors.centerIn: parent
        text: root.label
        font.family: Theme.typography.mono
        font.pixelSize: 10
        color: Theme.colors.muted
    }
}
