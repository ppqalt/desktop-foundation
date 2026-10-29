import QtQuick
import QtQuick.Controls
import "../theme"

Slider {
    id: root
    from: 0
    to: 100
    stepSize: 1
    activeFocusOnTab: true
    implicitHeight: 36
    Accessible.name: "Volume"
    background: Rectangle {
        x: root.leftPadding
        y: (root.height - height) / 2
        width: root.availableWidth
        height: 3
        radius: 1
        color: Theme.colors.border
        Rectangle {
            width: root.visualPosition * parent.width
            height: parent.height
            radius: 1
            color: Theme.colors.accent
        }
    }
    handle: Rectangle {
        x: root.leftPadding + root.visualPosition * (root.availableWidth - width)
        y: (root.height - height) / 2
        width: 16
        height: 16
        radius: 8
        color: Theme.colors.accent
        border.width: root.activeFocus ? 3 : 0
        border.color: Theme.colors.selectionBorder
    }
}
