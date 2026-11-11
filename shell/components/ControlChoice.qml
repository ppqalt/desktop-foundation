pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import "../theme"

// Exposed dropdown menu: M3 filled field (56dp), rounded top corners.
ComboBox {
    id: root
    implicitHeight: Math.max(56, Material.type.bodyLargeLine + 24)
    activeFocusOnTab: true
    property int menuIndex: 0
    property bool menuKeyboard: false
    property point lastMenuPointer: Qt.point(-1, -1)
    // Blend into a containing group without introducing a second filled surface.
    flat: false
    property string emptyText: "No options available"
    displayText: count > 0 ? currentText : emptyText
    PointerFocus {
        onFocusedByPointer: root.menuKeyboard = false
    }
    // Qt's native mouse path can open an empty ComboBox. Consume only that
    // unavailable state; populated menus keep native pointer handling.
    MouseArea {
        parent: root
        anchors.fill: parent
        enabled: root.count === 0
        acceptedButtons: Qt.LeftButton
        onPressed: Material.restoreFocus(root, Qt.MouseFocusReason)
    }
    onEnabledChanged: {
        if (!enabled && popup)
            popup.close();
    }
    onCountChanged: {
        menuIndex = count > 0 ? Math.max(0, Math.min(count - 1, menuIndex)) : -1;
        if (!count && popup)
            popup.close();
    }
    function handleKey(event): void {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
            return;
        const direction = Material.direction(event.key);
        const activate = [Qt.Key_Return, Qt.Key_Enter, Qt.Key_Space].includes(event.key);
        if (popup.visible) {
            if (direction && direction[1]) {
                menuKeyboard = true;
                menuIndex = Math.max(0, Math.min(count - 1, menuIndex + direction[1]));
                options.positionViewAtIndex(menuIndex, ListView.Contain);
                event.accepted = true;
            } else if (event.key === Qt.Key_Home || event.key === Qt.Key_End) {
                menuKeyboard = true;
                menuIndex = event.key === Qt.Key_Home ? 0 : count - 1;
                options.positionViewAtIndex(menuIndex, ListView.Contain);
                event.accepted = true;
            } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
                menuKeyboard = true;
                popup.close();
                event.accepted = false; // Continue normal focus traversal.
            } else if (activate) {
                menuKeyboard = true;
                if (!event.isAutoRepeat && count > 0) {
                    currentIndex = menuIndex;
                    activated(currentIndex);
                    popup.close();
                }
                event.accepted = true;
            } else if (event.key === Qt.Key_Escape) {
                menuKeyboard = true;
                if (!event.isAutoRepeat)
                    popup.close();
                event.accepted = true;
            }
        } else if (activate) {
            if (!event.isAutoRepeat && count > 0) {
                Material.restoreFocus(root, Qt.TabFocusReason);
                popup.open();
            }
            event.accepted = true;
        } else if (direction) {
            Material.restoreFocus(root, Qt.TabFocusReason);
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
        color: root.flat ? "transparent" : Material.colors.surface_container_highest
        opacity: root.enabled ? 1 : Material.state.disabledContainer
        StateLayer {
            cornerRadius: 4
            color: Material.colors.on_surface
            pressed: root.down
            focused: root.visualFocus
            hovered: root.hovered
        }
        Rectangle {
            visible: !root.flat || root.visualFocus || (root.popup.visible && root.menuKeyboard)
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: root.visualFocus || (root.popup.visible && root.menuKeyboard) ? 2 : 1
            color: root.visualFocus || (root.popup.visible && root.menuKeyboard) ? Material.colors.primary : Material.colors.on_surface_variant
        }
    }
    contentItem: Text {
        text: root.displayText
        textFormat: Text.PlainText
        verticalAlignment: Text.AlignVCenter
        color: Material.colors.on_surface
        opacity: root.enabled ? 1 : Material.state.disabledContent
        elide: Text.ElideRight
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyLarge
    }
    indicator: ControlIcon {
        x: root.width - width - 16
        anchors.verticalCenter: parent.verticalCenter
        name: "chevron_right"
        objectName: "choice-chevron"
        rotation: chevronMotion.value
        SpringValue {
            id: chevronMotion
            targetValue: root.popup.visible ? -90 : 90
        }
        color: Material.colors.on_surface_variant
        opacity: root.enabled ? 1 : Material.state.disabledContent
    }
    delegate: ItemDelegate {
        id: option
        required property var modelData
        required property int index
        width: root.width
        implicitHeight: Math.max(Material.control.target, Material.type.bodyLargeLine + 16)
        highlighted: root.menuIndex === index
        PointerFocus {
            onFocusedByPointer: {
                root.menuKeyboard = false;
                root.menuIndex = option.index;
            }
        }
        HoverHandler {
            parent: option
            cursorShape: Qt.PointingHandCursor
            function trackPointer(): void {
                if (!hovered || point.position.x < 0 || point.position.x > option.width || point.position.y < 0 || point.position.y > option.height)
                    return;
                const moved = root.lastMenuPointer.x >= 0 && Math.abs(root.lastMenuPointer.x - point.scenePosition.x) + Math.abs(root.lastMenuPointer.y - point.scenePosition.y) > 1;
                root.lastMenuPointer = point.scenePosition;
                if (moved) {
                    root.menuKeyboard = false;
                    root.menuIndex = option.index;
                }
            }
            onPointChanged: trackPointer()
            onHoveredChanged: {
                if (hovered)
                    trackPointer();
            }
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
            border.width: option.highlighted && root.menuKeyboard ? 2 : 0
            border.color: Material.colors.primary
            Behavior on color {
                SurfaceColorAnimation {}
            }
        }
    }
    popup: Popup {
        onAboutToShow: root.menuKeyboard = root.visualFocus
        onOpened: {
            root.menuIndex = Math.max(0, root.currentIndex);
            root.lastMenuPointer = Qt.point(-1, -1);
            options.positionViewAtIndex(root.menuIndex, ListView.Contain);
        }
        onClosed: {
            if (root.visible && root.enabled)
                Material.restoreFocus(root, root.menuKeyboard ? Qt.TabFocusReason : Qt.MouseFocusReason);
        }
        y: root.height + 4
        width: root.width
        padding: 8
        implicitHeight: Math.min(320, contentItem.implicitHeight + 16)
        background: SurfaceCard {
            radius: Material.shape.small
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
