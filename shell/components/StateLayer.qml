import QtQuick
import "../theme"

// Shared Material feedback; decoration only, never an input or focus owner.
Rectangle {
    property bool pressed: false
    property bool focused: false
    property bool hovered: false
    property real cornerRadius: Material.shape.medium
    anchors.fill: parent
    radius: cornerRadius
    opacity: !enabled ? 0 : pressed ? Material.state.pressed : focused ? Material.state.focus : hovered ? Material.state.hover : 0
    Behavior on opacity {
        SurfaceAnimation {
            duration: Material.motion.fastEffects
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Material.effectsCurve
        }
    }
}
