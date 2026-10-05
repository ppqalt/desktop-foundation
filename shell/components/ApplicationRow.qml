import QtQuick
import Quickshell
import "../theme"

Rectangle {
    id: root
    required property var entry
    required property bool selected
    property string iconSource: Quickshell.iconPath(root.entry.icon, true) || "../assets/application.svg"
    property string actionLabel: "↵"
    // Setting indicators use reported state; the caller confirms successful readback.
    property var settingState: null
    property string settingValue: ""
    property bool pending: false
    property string pendingLabel: "Saving…"
    property bool confirmed: false
    property bool interactive: true
    readonly property bool hasSettingState: typeof settingState === "boolean"
    readonly property bool hasSetting: hasSettingState || settingValue !== ""
    signal pointerMoved(real x, real y)
    signal hovered
    signal chosen
    implicitHeight: Theme.dimensions.rowHeight
    radius: Theme.radii.medium
    color: selected ? Theme.colors.selected : (mouse.containsMouse ? Theme.colors.hover : "transparent")
    border.width: selected || confirmed ? 1 : 0
    border.color: confirmed ? Theme.colors.accent : Theme.colors.selectionBorder
    Behavior on color {
        ColorAnimation {
            duration: Theme.timing.fast
        }
    }
    Behavior on border.color {
        ColorAnimation {
            duration: Theme.timing.normal
        }
    }
    Rectangle {
        width: 3
        height: parent.height - 24
        x: 1
        anchors.verticalCenter: parent.verticalCenter
        radius: 2
        color: Theme.colors.accent
        opacity: root.confirmed ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.timing.normal
            }
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
            id: icon
            anchors.centerIn: parent
            width: 26
            height: 26
            source: root.iconSource
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
        width: Math.max(0, indicators.x - x - 12)
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
    Row {
        id: indicators
        anchors.right: parent.right
        anchors.rightMargin: 20
        anchors.verticalCenter: parent.verticalCenter
        spacing: 9
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.hasSetting
            text: root.hasSettingState ? (root.settingState ? "On" : "Off") + (root.settingValue ? " · " + root.settingValue : "") : root.settingValue
            color: root.hasSettingState && !root.settingState ? Theme.colors.muted : Theme.colors.accent
            font.family: Theme.typography.family
            font.pixelSize: 12
            font.weight: Font.Medium
            width: Math.min(implicitWidth, Math.max(40, root.width * 0.28))
            elide: Text.ElideRight
        }
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.hasSettingState
            width: 34
            height: 18
            radius: 9
            color: root.settingState ? Theme.colors.selected : Theme.colors.iconTile
            border.width: 1
            border.color: root.settingState ? Theme.colors.accent : Theme.colors.border
            Rectangle {
                x: root.settingState ? 19 : 3
                y: 3
                width: 12
                height: 12
                radius: 6
                color: root.settingState ? Theme.colors.accent : Theme.colors.muted
                Behavior on x {
                    NumberAnimation {
                        duration: Theme.timing.fast
                        easing.type: Theme.easing
                    }
                }
            }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.pending ? root.pendingLabel : root.confirmed ? "✓" : root.actionLabel
            color: Theme.colors.accent
            font.family: Theme.typography.family
            font.pixelSize: text === "↵" ? 20 : 11
            visible: root.pending || root.confirmed || (!root.hasSetting && (root.selected || root.actionLabel !== "↵"))
        }
    }
    HoverHandler {
        enabled: root.interactive
        onPointChanged: {
            if (hovered)
                root.pointerMoved(point.scenePosition.x, point.scenePosition.y);
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.interactive
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor
        onEntered: root.hovered()
        onClicked: root.chosen()
    }
}
