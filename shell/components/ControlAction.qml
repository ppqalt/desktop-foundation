import QtQuick
import QtQuick.Controls
import "../theme"

AbstractButton {
    id: root
    property string detail: ""
    property string badge: ""
    property bool marked: false
    property bool compact: false
    implicitHeight: compact ? 42 : 56
    activeFocusOnTab: true
    Accessible.name: text + (detail ? ", " + detail : "") + (badge ? ", " + badge : "")
    Keys.onReturnPressed: event => {
        if (!event.isAutoRepeat)
            root.clicked();
        event.accepted = true;
    }
    Keys.onEnterPressed: event => {
        if (!event.isAutoRepeat)
            root.clicked();
        event.accepted = true;
    }
    Keys.onUpPressed: event => {
        nextItemInFocusChain(false).forceActiveFocus(Qt.BacktabFocusReason);
        event.accepted = true;
    }
    Keys.onDownPressed: event => {
        nextItemInFocusChain(true).forceActiveFocus(Qt.TabFocusReason);
        event.accepted = true;
    }
    background: Rectangle {
        radius: Theme.radii.small
        color: root.marked || root.down ? Theme.colors.selected : root.hovered ? Theme.colors.hover : "transparent"
        border.width: root.activeFocus || root.marked ? 1 : 0
        border.color: root.activeFocus ? Theme.colors.accent : Theme.colors.selectionBorder
        opacity: root.enabled ? 1 : Theme.opacity.disabled
    }
    contentItem: Item {
        implicitWidth: 150
        opacity: root.enabled ? 1 : Theme.opacity.disabled
        Column {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.right: indicator.left
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 3
            Text {
                width: parent.width
                text: root.text
                textFormat: Text.PlainText
                color: Theme.colors.foreground
                elide: Text.ElideRight
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.body
            }
            Text {
                width: parent.width
                visible: root.detail !== ""
                text: root.detail
                textFormat: Text.PlainText
                color: Theme.colors.muted
                elide: Text.ElideRight
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.small
            }
        }
        Text {
            id: indicator
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: root.badge
            textFormat: Text.PlainText
            color: root.marked ? Theme.colors.accent : Theme.colors.muted
            font.family: Theme.typography.mono
            font.pixelSize: Theme.typography.small
        }
    }
}
