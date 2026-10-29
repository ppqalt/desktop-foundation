import QtQuick
import QtQuick.Controls
import "../theme"

// M3 Expressive small slider: 24dp track, 44x4dp handle, 8dp corners,
// 6dp handle/track gap, 48dp minimum interactive height. No value animation.
Slider {
    id: root
    from: 0
    to: 100
    stepSize: 1
    padding: 6
    activeFocusOnTab: true
    implicitHeight: 48
    Accessible.name: "Volume"
    Keys.onPressed: event => {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
            return;
        const direction = Material.direction(event.key);
        if (!direction)
            return;
        if (direction[0]) {
            const previous = value;
            if (direction[0] > 0)
                increase();
            else
                decrease();
            if (value !== previous)
                moved();
        } else
            Material.moveFocus(root, direction[0], direction[1]);
        event.accepted = true;
    }
    readonly property real handleCenter: leftPadding + 2 + visualPosition * (availableWidth - 4)
    readonly property color activeColor: enabled ? Material.colors.primary : Material.colors.on_surface
    background: Item {
        x: root.leftPadding
        y: (root.height - height) / 2
        width: root.availableWidth
        height: 24
        opacity: root.enabled ? 1 : 0.12
        Rectangle {
            width: Math.max(0, root.handleCenter - root.leftPadding - 8)
            height: parent.height
            radius: 8
            color: root.activeColor
        }
        Rectangle {
            x: root.handleCenter - root.leftPadding + 8
            width: Math.max(0, parent.width - x)
            height: parent.height
            radius: 8
            color: Material.colors.secondary_container
            Rectangle {
                anchors.right: parent.right
                anchors.rightMargin: 4
                anchors.verticalCenter: parent.verticalCenter
                width: 4
                height: 4
                radius: 2
                visible: parent.width > 12
                color: Material.colors.on_secondary_container
            }
        }
    }
    handle: Rectangle {
        x: root.handleCenter - width / 2
        y: (root.height - height) / 2
        width: root.pressed ? 2 : 4
        height: 44
        radius: 2
        color: root.activeColor
        opacity: root.enabled ? 1 : 0.38
        Rectangle {
            anchors.fill: parent
            anchors.margins: -4
            color: "transparent"
            radius: 6
            border.width: 3
            border.color: Material.colors.primary
            visible: root.activeFocus
        }
    }
}
