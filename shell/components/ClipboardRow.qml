import QtQuick
import "../theme"

Rectangle {
    id: root
    required property var entry
    required property bool selected
    signal chosen
    signal removed
    implicitHeight: Theme.dimensions.rowHeight
    radius: Theme.radii.medium
    color: selected ? Theme.colors.selected : (mouse.containsMouse ? Theme.colors.hover : "transparent")
    border.width: selected ? 1 : 0
    border.color: Theme.colors.selectionBorder
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
        color: Theme.colors.iconTile
        Image {
            anchors.fill: parent
            anchors.margins: 3
            source: root.entry.image
            fillMode: Image.PreserveAspectFit
            sourceSize.width: 72
            sourceSize.height: 72
            visible: !!root.entry.image
        }
        Text {
            anchors.centerIn: parent
            text: "Aa"
            visible: !root.entry.image
            color: Theme.colors.accent
            font.family: Theme.typography.family
            font.pixelSize: 15
            font.weight: Font.Medium
        }
    }
    Column {
        x: 66
        width: parent.width - 146
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Text {
            width: parent.width
            text: root.entry.image ? "Copied image" : root.entry.preview.replace(/\s+/g, " ").trim()
            textFormat: Text.PlainText
            color: Theme.colors.foreground
            font.family: Theme.typography.family
            font.pixelSize: 14
            font.weight: Font.Medium
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            text: (root.entry.image ? root.entry.mime.split("/")[1].toUpperCase() : "Text") + " · " + (root.entry.size >= 1024 ? (root.entry.size / 1024).toFixed(1) + " KB" : root.entry.size + " bytes") + (root.entry.preview.includes("\n") && !root.entry.image ? " · multiline" : "")
            color: Theme.colors.muted
            font.family: Theme.typography.family
            font.pixelSize: 11
            elide: Text.ElideRight
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.chosen()
    }
    Text {
        anchors.right: parent.right
        anchors.rightMargin: 52
        anchors.verticalCenter: parent.verticalCenter
        text: "↵"
        color: Theme.colors.accent
        font.pixelSize: 20
        visible: root.selected
    }
    Rectangle {
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        width: 30
        height: 30
        radius: 8
        color: deleteMouse.containsMouse ? Theme.colors.elevated : "transparent"
        Text {
            anchors.centerIn: parent
            text: "×"
            font.pixelSize: 19
            color: deleteMouse.containsMouse ? Theme.colors.error : Theme.colors.subtle
        }
        MouseArea {
            id: deleteMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.removed()
        }
    }
}
