import QtQuick
import Quickshell
import "../theme"

Rectangle {
    id: root
    required property var entry
    required property bool selected
    signal chosen
    implicitHeight: Theme.dimensions.rowHeight
    radius: Theme.radii.medium
    color: selected ? Theme.colors.selected : (mouse.containsMouse ? Theme.colors.hover : "transparent")
    border.width: selected ? 1 : 0
    border.color: "#485a73"
    Behavior on color {
        ColorAnimation {
            duration: Theme.timing.fast
        }
    }
    Rectangle {
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: 38
        height: 38
        radius: 10
        color: "#172029"
        Image {
            id: icon
            anchors.centerIn: parent
            width: 26
            height: 26
            source: Quickshell.iconPath(root.entry.icon, true) || "../assets/application.svg"
            sourceSize.width: 32
            sourceSize.height: 32
            smooth: true
            visible: status !== Image.Error
        }
        Image {
            anchors.centerIn: parent
            width: 26
            height: 26
            source: "../assets/application.svg"
            visible: icon.status === Image.Error
        }
    }
    Column {
        x: 66
        width: parent.width - 130
        anchors.verticalCenter: parent.verticalCenter
        spacing: 3
        Text {
            width: parent.width
            text: root.entry.name
            color: Theme.colors.foreground
            font.family: Theme.typography.family
            font.pixelSize: 14
            font.weight: Font.Medium
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            text: root.entry.genericName || root.entry.comment || "Application"
            color: Theme.colors.muted
            font.family: Theme.typography.family
            font.pixelSize: 11
            elide: Text.ElideRight
        }
    }
    Text {
        anchors.right: parent.right
        anchors.rightMargin: 20
        anchors.verticalCenter: parent.verticalCenter
        text: "↵"
        color: Theme.colors.accent
        font.pixelSize: 20
        visible: root.selected
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.chosen()
    }
}
