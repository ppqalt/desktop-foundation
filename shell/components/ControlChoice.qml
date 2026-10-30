pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import "../theme"

// Exposed dropdown menu: M3 filled field (56dp), rounded top corners.
ComboBox {
    id: root
    implicitHeight: 56
    activeFocusOnTab: true
    property int menuIndex: 0
    function handleKey(event): void {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
            return;
        const direction = Material.direction(event.key);
        const activate = [Qt.Key_Return, Qt.Key_Enter, Qt.Key_Space].includes(event.key);
        if (popup.visible) {
            if (direction && direction[1]) {
                menuIndex = Math.max(0, Math.min(count - 1, menuIndex + direction[1]));
                options.positionViewAtIndex(menuIndex, ListView.Contain);
                event.accepted = true;
            } else if (activate) {
                if (!event.isAutoRepeat && count > 0) {
                    currentIndex = menuIndex;
                    activated(currentIndex);
                    popup.close();
                }
                event.accepted = true;
            } else if (event.key === Qt.Key_Escape) {
                popup.close();
                event.accepted = true;
            }
        } else if (activate) {
            if (!event.isAutoRepeat)
                popup.open();
            event.accepted = true;
        } else if (direction) {
            Material.moveFocus(root, direction[0], direction[1]);
            event.accepted = true;
        }
    }
    Keys.priority: Keys.BeforeItem
    Keys.onPressed: event => handleKey(event)
    leftPadding: 16
    rightPadding: 48
    background: Rectangle {
        topLeftRadius: 4
        topRightRadius: 4
        color: Material.colors.surface_container_highest
        opacity: root.enabled ? 1 : 0.12
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: root.activeFocus ? 2 : 1
            color: root.activeFocus ? Material.colors.primary : Material.colors.on_surface_variant
        }
    }
    contentItem: Text {
        text: root.displayText
        textFormat: Text.PlainText
        verticalAlignment: Text.AlignVCenter
        color: Material.colors.on_surface
        opacity: root.enabled ? 1 : 0.38
        elide: Text.ElideRight
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyLarge
    }
    indicator: ControlIcon {
        x: root.width - width - 16
        anchors.verticalCenter: parent.verticalCenter
        name: "chevron_right"
        rotation: root.popup.visible ? -90 : 90
        color: Material.colors.on_surface_variant
    }
    delegate: ItemDelegate {
        id: option
        required property var modelData
        required property int index
        width: root.width
        implicitHeight: 48
        highlighted: root.menuIndex === index
        onHoveredChanged: {
            if (hovered)
                root.menuIndex = index;
        }
        leftPadding: 12
        rightPadding: 12
        contentItem: Text {
            text: (option.index === root.currentIndex ? "✓  " : "") + option.modelData[root.textRole]
            textFormat: Text.PlainText
            verticalAlignment: Text.AlignVCenter
            color: option.index === root.currentIndex ? Material.colors.on_secondary_container : Material.colors.on_surface
            elide: Text.ElideRight
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyLarge
        }
        background: Rectangle {
            radius: 4
            color: option.index === root.currentIndex ? Material.colors.secondary_container : option.highlighted ? Material.colors.surface_container_highest : "transparent"
            border.width: option.highlighted ? 2 : 0
            border.color: Material.colors.primary
        }
    }
    popup: Popup {
        onOpened: root.menuIndex = Math.max(0, root.currentIndex)
        onClosed: {
            if (root.visible && root.enabled)
                root.forceActiveFocus(Qt.PopupFocusReason);
        }
        y: root.height + 4
        width: root.width
        padding: 8
        implicitHeight: Math.min(320, contentItem.implicitHeight + 16)
        background: Rectangle {
            radius: 4
            color: Material.colors.surface_container
        }
        contentItem: ListView {
            id: options
            Keys.onPressed: event => root.handleKey(event)
            clip: true
            implicitHeight: contentHeight
            model: root.popup.visible ? root.delegateModel : null
            currentIndex: root.menuIndex
            ScrollIndicator.vertical: ScrollIndicator {}
        }
    }
}
