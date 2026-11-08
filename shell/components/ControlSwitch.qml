import QtQuick
import QtQuick.Controls
import "../theme"

// Geometry/roles: Google's Material Web switch tokens (v0_192).
// https://github.com/material-components/material-web/blob/main/tokens/versions/v0_192/_md-comp-switch.scss
// Marked is backend readback; clicking requests a change without toggling local truth.
AbstractButton {
    id: root
    property bool marked: false
    property bool selected: false
    property bool pending: false
    signal pointerMoved(real x, real y)
    implicitWidth: Material.control.switchWidth
    implicitHeight: Material.control.target
    activeFocusOnTab: true
    checkable: false
    Keys.onPressed: event => {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
            return;
        const direction = Material.direction(event.key);
        if (direction) {
            Material.moveFocus(root, direction[0], direction[1]);
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.CheckBox
    Accessible.name: text
    Accessible.checkable: true
    Accessible.checked: marked
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
    background: Item {
        Rectangle {
            id: track
            objectName: "control-switch-track"
            anchors.centerIn: parent
            width: Material.control.switchTrackWidth
            height: Material.control.switchTrackHeight
            radius: height / 2
            color: root.marked ? Material.colors.primary : Material.colors.surface_container_highest
            border.width: root.marked ? 0 : 2
            border.color: Material.colors.outline
            opacity: root.enabled ? 1 : 0.38
            Rectangle {
                id: thumb
                objectName: "control-switch-thumb"
                width: root.down ? 28 : root.marked ? 24 : 16
                height: width
                radius: width / 2
                x: root.marked ? parent.width - width - (root.down ? 2 : 4) : root.down ? 2 : 8
                anchors.verticalCenter: parent.verticalCenter
                color: root.marked ? Material.colors.on_primary : Material.colors.outline
                Behavior on x {
                    SurfaceAnimation {
                        duration: Material.motion.fastEffects
                    }
                }
                Behavior on width {
                    SurfaceAnimation {
                        duration: Material.motion.fastEffects
                    }
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
        FocusIndicator {
            anchors.fill: parent
            cornerRadius: height / 2
            visible: root.selected || root.activeFocus
        }
    }
    contentItem: Item {}
    HoverHandler {
        cursorShape: root.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onPointChanged: {
            if (hovered)
                root.pointerMoved(point.scenePosition.x, point.scenePosition.y);
        }
    }
}
