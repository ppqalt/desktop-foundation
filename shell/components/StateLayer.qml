import QtQuick
import "../theme"

// Shared Material feedback; decoration only, never an input or focus owner.
Rectangle {
    id: root
    property bool pressed: false
    property bool focused: false
    property bool hovered: false
    readonly property bool pulse: activation.active
    InteractionPulse {
        id: activation
        pressed: root.pressed
        enabled: root.enabled
    }
    property real cornerRadius: Material.shape.medium
    anchors.fill: parent
    radius: cornerRadius
    opacity: !enabled ? 0 : pressed || pulse ? Material.state.pressed : focused ? Material.state.focus : hovered ? Material.state.hover : 0
    Behavior on opacity {
        SurfaceAnimation {
            duration: Material.motion.fastEffects
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Material.effectsCurve
        }
    }
}
