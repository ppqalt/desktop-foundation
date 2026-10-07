import QtQuick
import QtQuick.Controls as Controls
import "../theme"

Rectangle {
    id: root
    required property var entry
    required property bool selected
    property int actionFocus: 0
    property bool busy: false
    readonly property string kind: entry.kind || (entry.image ? "image" : (["text/html", "text/rtf", "application/rtf"].includes(entry.mime) ? "rich" : "text"))
    readonly property bool plainAvailable: kind === "rich"
    readonly property int actionInset: plainAvailable ? 0 : 38
    readonly property string title: entry.label || (kind === "image" ? "Copied image" : (entry.preview || "Clipboard item").replace(/\s+/g, " ").trim())
    readonly property string detail: entry.unavailable ? "Original file is unavailable" : (entry.detail || ((kind === "rich" ? "Formatted text" : (kind === "files" ? (entry.fileCount || 1) + " file" + (entry.fileCount > 1 ? "s" : "") : (kind === "image" ? (entry.mime || "image").split("/")[1].toUpperCase() : "Text"))) + " · " + (entry.size >= 1024 ? (entry.size / 1024).toFixed(1) + " KB" : (entry.size || 0) + " bytes")))
    signal chosen
    signal pinned
    signal plainCopied
    implicitHeight: Theme.dimensions.rowHeight
    radius: Theme.radii.medium
    color: selected ? Theme.colors.selected : (mouse.containsMouse ? Theme.colors.hover : "transparent")
    border.width: selected ? 1 : 0
    border.color: Theme.colors.selectionBorder
    Accessible.role: Accessible.ListItem
    Accessible.name: title + ". " + detail + (entry.pinned ? ". Pinned" : "")
    Accessible.onPressAction: root.chosen()
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
            source: root.kind === "image" ? (root.entry.image || "") : ""
            fillMode: Image.PreserveAspectFit
            sourceSize.width: 72
            sourceSize.height: 72
            visible: !!source
        }
        Text {
            anchors.centerIn: parent
            text: root.kind === "files" ? ((root.entry.detail || "").toLocaleLowerCase().includes("folder") ? "▱" : "▤") : (root.kind === "rich" ? "≡" : (root.kind === "image" ? "▣" : "Aa"))
            visible: root.kind !== "image" || !root.entry.image
            color: root.entry.unavailable ? Theme.colors.error : Theme.colors.accent
            font.family: Theme.typography.family
            font.pixelSize: root.kind === "text" ? 15 : 22
            font.weight: Font.Medium
        }
    }
    Column {
        x: 66
        width: parent.width - 186 + root.actionInset
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Text {
            width: parent.width
            text: root.title
            textFormat: Text.PlainText
            color: Theme.colors.foreground
            font.family: Theme.typography.family
            font.pixelSize: 14
            font.weight: Font.Medium
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            text: (root.entry.pinned ? "Pinned · " : "") + root.detail
            textFormat: Text.PlainText
            color: root.entry.unavailable ? Theme.colors.error : (root.entry.pinned ? Theme.colors.accent : Theme.colors.muted)
            font.family: Theme.typography.family
            font.pixelSize: 11
            elide: Text.ElideRight
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        enabled: !root.busy
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.chosen()
    }
    Controls.ToolTip {
        id: filesTip
        width: Math.min(420, root.width - 32)
        visible: root.kind === "files" && mouse.containsMouse && !pinMouse.containsMouse && !plainMouse.containsMouse
        delay: 500
        text: (root.entry.preview || root.title).slice(0, 512)
        background: Rectangle {
            color: Theme.colors.elevated
            radius: Theme.radii.small
            border.color: Theme.colors.border
        }
        contentItem: Text {
            text: filesTip.text
            textFormat: Text.PlainText
            color: Theme.colors.foreground
            font.family: Theme.typography.family
            font.pixelSize: 11
            wrapMode: Text.WrapAnywhere
        }
    }
    Text {
        anchors.right: parent.right
        anchors.rightMargin: 92 - root.actionInset
        anchors.verticalCenter: parent.verticalCenter
        text: "↵"
        color: Theme.colors.accent
        font.pixelSize: 20
        visible: root.selected && root.actionFocus === 0
    }
    Rectangle {
        id: pinButton
        anchors.right: parent.right
        anchors.rightMargin: 50 - root.actionInset
        anchors.verticalCenter: parent.verticalCenter
        width: 30
        height: 30
        radius: Theme.radii.small
        color: root.selected && root.actionFocus === 1 ? Theme.colors.iconTile : (pinMouse.containsMouse ? Theme.colors.hover : "transparent")
        border.width: root.selected && root.actionFocus === 1 ? 1 : 0
        border.color: Theme.colors.accent
        enabled: !root.busy
        Accessible.role: Accessible.Button
        Accessible.name: root.entry.pinned ? "Unpin item" : "Pin item"
        Accessible.onPressAction: root.pinned()
        Canvas {
            id: pinCanvas
            anchors.centerIn: parent
            width: 16
            height: 18
            property color ink: root.entry.pinned || pinMouse.containsMouse || (root.selected && root.actionFocus === 1) ? Theme.colors.accent : Theme.colors.subtle
            onInkChanged: requestPaint()
            onPaint: {
                const painter = getContext("2d");
                painter.reset();
                painter.strokeStyle = ink;
                painter.fillStyle = root.entry.pinned ? ink : "transparent";
                painter.lineWidth = 1.5;
                painter.lineJoin = "round";
                painter.beginPath();
                painter.moveTo(4, 2);
                painter.lineTo(12, 2);
                painter.lineTo(10, 5);
                painter.lineTo(10, 8);
                painter.lineTo(13, 11);
                painter.lineTo(3, 11);
                painter.lineTo(6, 8);
                painter.lineTo(6, 5);
                painter.closePath();
                painter.fill();
                painter.stroke();
                painter.beginPath();
                painter.moveTo(8, 11);
                painter.lineTo(8, 17);
                painter.stroke();
            }
            Connections {
                target: root
                function onEntryChanged(): void {
                    pinCanvas.requestPaint();
                }
            }
        }
        MouseArea {
            id: pinMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.pinned()
        }
        Controls.ToolTip {
            id: pinTip
            visible: pinMouse.containsMouse
            delay: 400
            text: (root.entry.pinned ? "Unpin" : "Pin") + " · Ctrl+Shift+P"
            background: Rectangle {
                color: Theme.colors.elevated
                radius: Theme.radii.small
                border.color: Theme.colors.border
            }
            contentItem: Text {
                text: pinTip.text
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: 11
            }
        }
    }
    Rectangle {
        id: plainButton
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        width: 30
        height: 30
        radius: Theme.radii.small
        visible: root.plainAvailable
        color: root.selected && root.actionFocus === 2 ? Theme.colors.iconTile : (plainMouse.containsMouse ? Theme.colors.hover : "transparent")
        border.width: root.selected && root.actionFocus === 2 ? 1 : 0
        border.color: Theme.colors.accent
        enabled: !root.busy
        Accessible.role: Accessible.Button
        Accessible.name: "Copy as plain text"
        Accessible.onPressAction: root.plainCopied()
        Text {
            anchors.centerIn: parent
            text: "T"
            font.family: Theme.typography.family
            font.pixelSize: 14
            font.underline: true
            color: plainMouse.containsMouse || (root.selected && root.actionFocus === 2) ? Theme.colors.accent : Theme.colors.subtle
        }
        MouseArea {
            id: plainMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.plainCopied()
        }
        Controls.ToolTip {
            id: plainTip
            visible: plainMouse.containsMouse
            delay: 400
            text: "Copy plain text · Ctrl+Shift+C"
            background: Rectangle {
                color: Theme.colors.elevated
                radius: Theme.radii.small
                border.color: Theme.colors.border
            }
            contentItem: Text {
                text: plainTip.text
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: 11
            }
        }
    }
}
