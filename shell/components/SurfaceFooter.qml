import QtQuick
import "../theme"

Item {
    id: root
    objectName: "surface-footer"
    property string navigationKeys: "↑ ↓"
    property string navigationLabel: "navigate"
    property string escapeLabel: "close"
    property bool escapeEnabled: false
    property bool adjustable: false
    property bool hintsVisible: false
    property string actionText: ""
    property string actionKey: "↵"
    property bool actionEnabled: false
    signal activated
    signal escapeRequested
    visible: hintsVisible || actionText !== ""
    implicitHeight: visible ? 48 : 0
    Rectangle {
        width: parent.width
        height: 1
        color: "transparent"
    }
    Row {
        id: hints
        y: 13
        spacing: 7
        visible: root.hintsVisible
        Keycap {
            label: root.navigationKeys
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.navigationLabel
            color: Material.colors.on_surface_variant
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
        }
        Item {
            width: 8
            height: 1
        }
        Keycap {
            visible: root.adjustable && root.width >= 480
            label: "← →"
        }
        Text {
            visible: root.adjustable && root.width >= 480
            anchors.verticalCenter: parent.verticalCenter
            text: "adjust"
            color: Material.colors.on_surface_variant
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
        }
        Item {
            objectName: "surface-footer-escape"
            width: escapeHint.width
            height: 22
            Row {
                id: escapeHint
                spacing: 7
                Keycap {
                    label: "esc"
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.escapeLabel
                    color: Material.colors.on_surface_variant
                    font.family: Theme.typography.family
                    font.pixelSize: Theme.typography.small
                }
            }
            MouseArea {
                anchors.fill: parent
                anchors.margins: -4
                enabled: root.escapeEnabled
                cursorShape: Qt.PointingHandCursor
                onClicked: root.escapeRequested()
            }
        }
    }
    Item {
        id: actionArea
        objectName: "surface-footer-action"
        anchors.right: parent.right
        y: 13
        width: action.width
        height: 22
        visible: root.actionText !== ""
        Row {
            id: action
            spacing: 7
            Text {
                width: Math.min(implicitWidth, Math.max(0, root.width - (root.hintsVisible ? hints.width : 0) - keycap.width - action.spacing - 16))
                anchors.verticalCenter: parent.verticalCenter
                text: root.actionText
                color: Material.colors.primary
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.small
                elide: Text.ElideRight
            }
            Keycap {
                id: keycap
                label: root.actionKey
            }
        }
        MouseArea {
            anchors.fill: parent
            anchors.margins: -4
            enabled: root.actionEnabled
            cursorShape: Qt.PointingHandCursor
            onClicked: root.activated()
        }
    }
}
