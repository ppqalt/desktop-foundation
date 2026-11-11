import QtQuick
import "../theme"

// Shared Material feedback; decoration only, never an input or focus owner.
Rectangle {
    id: root
    property bool pressed: false
    property bool focused: false
    property bool hovered: false
    property bool pulse: false
    onPressedChanged: {
        if (pressed && !Theme.reducedMotion) {
            pulse = true;
            pulseClock.restart();
        }
    }
    onEnabledChanged: {
        if (!enabled) {
            pulseClock.stop();
            pulse = false;
        }
    }
    Timer {
        id: pulseClock
        interval: 90
        onTriggered: root.pulse = false
    }
    Connections {
        target: Theme
        function onReducedMotionChanged(): void {
            if (Theme.reducedMotion) {
                pulseClock.stop();
                root.pulse = false;
            }
        }
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
