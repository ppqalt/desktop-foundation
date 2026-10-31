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
    implicitHeight: 72
    radius: Theme.radii.medium
    color: selected ? Material.colors.secondary_container : (mouse.containsMouse ? Material.colors.surface_container_high : "transparent")
    border.width: selected ? 1 : 0
    border.color: Material.colors.outline
    Accessible.role: Accessible.ListItem
    Accessible.name: title + ". " + detail + (entry.pinned ? ". Pinned" : "")
    Accessible.onPressAction: root.chosen()
    Behavior on color {
        ColorAnimation {
            duration: Material.motion.fastEffects
        }
    }
    Rectangle {
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: 38
        height: 38
        radius: 10
        color: Material.colors.surface_container_highest
        Image {
            anchors.fill: parent
            anchors.margins: 3
            source: root.kind === "image" ? (root.entry.image || "") : ""
            fillMode: Image.PreserveAspectFit
            sourceSize.width: 72
            sourceSize.height: 72
            asynchronous: true
            visible: !!source
        }
        Text {
            anchors.centerIn: parent
            text: root.kind === "files" ? ((root.entry.detail || "").toLocaleLowerCase().includes("folder") ? "▱" : "▤") : (root.kind === "rich" ? "≡" : (root.kind === "image" ? "▣" : "Aa"))
            visible: root.kind !== "image" || !root.entry.image
            color: root.entry.unavailable ? Material.colors.error : Material.colors.primary
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
            color: root.selected ? Material.colors.on_secondary_container : Material.colors.on_surface
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyLarge
            font.weight: Font.Medium
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            text: root.detail
            textFormat: Text.PlainText
            color: root.entry.unavailable ? Material.colors.error : (root.entry.pinned ? Material.colors.primary : Material.colors.on_surface_variant)
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
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
            color: Material.colors.surface_container_high
            radius: Theme.radii.small
            border.color: Material.colors.outline_variant
        }
        contentItem: Text {
            text: filesTip.text
            textFormat: Text.PlainText
            color: Material.colors.on_surface
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
            wrapMode: Text.WrapAnywhere
        }
    }
    Text {
        anchors.right: parent.right
        anchors.rightMargin: 92 - root.actionInset
        anchors.verticalCenter: parent.verticalCenter
        text: "↵"
        color: Material.colors.primary
        font.pixelSize: 20
        visible: root.selected && root.actionFocus === 0
    }
    Rectangle {
        id: pinButton
        anchors.right: parent.right
        anchors.rightMargin: 60 - root.actionInset
        anchors.verticalCenter: parent.verticalCenter
        width: 48
        height: 48
        radius: 24
        color: root.selected && root.actionFocus === 1 ? Material.colors.surface_container_highest : (pinMouse.containsMouse ? Material.colors.surface_container_high : "transparent")
        border.width: root.selected && root.actionFocus === 1 ? 1 : 0
        border.color: Material.colors.primary
        enabled: !root.busy
        Accessible.role: Accessible.Button
        Accessible.name: root.entry.pinned ? "Unpin item" : "Pin item"
        Accessible.onPressAction: root.pinned()
        Canvas {
            id: pinCanvas
            anchors.centerIn: parent
            width: 16
            height: 18
            property color ink: root.entry.pinned || pinMouse.containsMouse || (root.selected && root.actionFocus === 1) ? Material.colors.primary : Material.colors.on_surface_variant
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
                color: Material.colors.surface_container_high
                radius: Theme.radii.small
                border.color: Material.colors.outline_variant
            }
            contentItem: Text {
                text: pinTip.text
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
            }
        }
    }
    Rectangle {
        id: plainButton
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        width: 48
        height: 48
        radius: 24
        visible: root.plainAvailable
        color: root.selected && root.actionFocus === 2 ? Material.colors.surface_container_highest : (plainMouse.containsMouse ? Material.colors.surface_container_high : "transparent")
        border.width: root.selected && root.actionFocus === 2 ? 1 : 0
        border.color: Material.colors.primary
        enabled: !root.busy
        Accessible.role: Accessible.Button
        Accessible.name: "Copy as plain text"
        Accessible.onPressAction: root.plainCopied()
        Text {
            anchors.centerIn: parent
            text: "T"
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyLarge
            font.underline: true
            color: plainMouse.containsMouse || (root.selected && root.actionFocus === 2) ? Material.colors.primary : Material.colors.on_surface_variant
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
                color: Material.colors.surface_container_high
                radius: Theme.radii.small
                border.color: Material.colors.outline_variant
            }
            contentItem: Text {
                text: plainTip.text
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
            }
        }
    }
}
