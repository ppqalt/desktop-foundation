import QtQuick
import "../theme"

// Geometry/roles: Google's Material Web switch tokens (v0_192).
// https://github.com/material-components/material-web/blob/main/tokens/versions/v0_192/_md-comp-switch.scss
// Marked is backend readback; clicking requests a change without toggling local truth.
ControlButton {
    id: root
    property bool marked: false
    property bool selected: false
    property bool keyboardSelection: false
    property bool pending: false
    signal pointerMoved(real x, real y)
    implicitWidth: Material.control.switchWidth
    implicitHeight: Material.control.target
    checkable: false
    Accessible.role: Accessible.CheckBox
    Accessible.name: text
    Accessible.checkable: true
    Accessible.checked: marked
    background: Item {
        SwitchIndicator {
            anchors.centerIn: parent
            marked: root.marked
            pressed: root.down
            pending: root.pending
        }
        FocusIndicator {
            anchors.fill: parent
            cornerRadius: height / 2
            visible: (root.selected && root.keyboardSelection) || root.visualFocus
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
