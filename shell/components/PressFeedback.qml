import QtQuick
import "../theme"

// Presentation only: a fixed-pixel compression, independent of control width.
// Never delay activation or transform a slider's pointer/value geometry.
Scale {
    id: root
    required property Item control
    property bool pressed: false
    readonly property bool available: control.enabled && control.visible
    readonly property bool pulsing: activation.active
    readonly property real compression: Math.max(0, travel.value)
    origin.x: control.width / 2
    origin.y: control.height / 2
    xScale: 1 - Math.min(0.08, compression / Math.max(1, control.width))
    yScale: 1 - Math.min(0.08, compression / Math.max(1, control.height))
    function activate(): void {
        activation.trigger();
    }
    property SpringValue travel: SpringValue {
        targetValue: root.available && !Theme.reducedMotion && (root.pressed || root.pulsing) ? Material.motionDistance.press : 0
    }
    property InteractionPulse activation: InteractionPulse {
        pressed: root.pressed
        enabled: root.available
    }
}
