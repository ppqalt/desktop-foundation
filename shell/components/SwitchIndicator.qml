import QtQuick
import "../theme"

// One switch drawing and motion implementation; no input or backend ownership.
Rectangle {
    id: root
    property bool marked: false
    property bool pressed: false
    property bool pending: false
    objectName: "control-switch-track"
    width: Material.control.switchTrackWidth
    height: Material.control.switchTrackHeight
    radius: height / 2
    color: root.marked ? Material.colors.primary : Material.colors.surface_container_highest
    border.width: root.marked ? 0 : 2
    border.color: Material.colors.outline
    opacity: root.enabled ? 1 : Material.state.disabledContent
    Behavior on color {
        SurfaceColorAnimation {}
    }
    StateLayer {
        color: Material.colors.on_surface
        pressed: root.pressed
        cornerRadius: root.radius
    }
    Rectangle {
        id: thumb
        objectName: "control-switch-thumb"
        width: Math.max(0, thumbSize.value)
        height: width
        radius: width / 2
        x: thumbPosition.value
        anchors.verticalCenter: parent.verticalCenter
        color: root.marked ? Material.colors.on_primary : Material.colors.outline
        SpringValue {
            id: thumbSize
            targetValue: root.pressed ? 28 : root.marked ? 24 : 16
        }
        SpringValue {
            id: thumbPosition
            targetValue: root.marked ? root.width - thumbSize.targetValue - (root.pressed ? 2 : 4) : root.pressed ? 2 : 8
        }
        Behavior on color {
            SurfaceColorAnimation {}
        }
        ControlIcon {
            anchors.centerIn: parent
            name: root.pending ? "more_horiz" : "check"
            width: 16
            height: 16
            visible: root.marked || root.pending
            color: root.marked ? Material.colors.on_primary_container : Material.colors.surface_container_highest
        }
    }
}
