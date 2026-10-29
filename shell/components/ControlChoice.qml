pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import "../theme"

ComboBox {
    id: root
    implicitHeight: 46
    activeFocusOnTab: true
    leftPadding: 12
    rightPadding: 32
    background: Rectangle {
        radius: Theme.radii.small
        color: Theme.colors.iconTile
        border.width: 1
        border.color: root.activeFocus ? Theme.colors.accent : Theme.colors.border
        opacity: root.enabled ? 1 : Theme.opacity.disabled
    }
    contentItem: Text {
        text: root.displayText
        textFormat: Text.PlainText
        verticalAlignment: Text.AlignVCenter
        color: root.enabled ? Theme.colors.foreground : Theme.colors.muted
        elide: Text.ElideRight
        font.family: Theme.typography.family
        font.pixelSize: Theme.typography.body
    }
    indicator: Text {
        x: root.width - width - 12
        anchors.verticalCenter: parent.verticalCenter
        text: "⌄"
        color: Theme.colors.accent
        font.pixelSize: 18
    }
    delegate: ItemDelegate {
        id: option
        required property var modelData
        required property int index
        width: root.width
        implicitHeight: 44
        highlighted: root.highlightedIndex === index
        contentItem: Text {
            text: option.modelData[root.textRole]
            textFormat: Text.PlainText
            color: Theme.colors.foreground
            elide: Text.ElideRight
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.body
        }
        background: Rectangle {
            radius: Theme.radii.small
            color: option.highlighted ? Theme.colors.selected : "transparent"
        }
    }
    popup: Popup {
        y: root.height + 4
        width: root.width
        padding: 6
        implicitHeight: Math.min(260, contentItem.implicitHeight + 12)
        background: Rectangle {
            radius: Theme.radii.medium
            color: Theme.colors.elevated
            border.width: 1
            border.color: Theme.colors.border
        }
        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: root.popup.visible ? root.delegateModel : null
            currentIndex: root.highlightedIndex
            ScrollIndicator.vertical: ScrollIndicator {}
        }
    }
}
