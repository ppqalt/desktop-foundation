import QtQuick
import "../theme"

// A unit-mass damped spring. Retargeting retains position AND velocity.
// Exact time integration makes completion independent of refresh rate/frame loss.
QtObject {
    id: root
    property real targetValue: 0
    property string motion: "fastSpatial"
    property real tolerance: 0.001
    readonly property var spec: Material.springs[motion] ?? Material.springs.fastSpatial
    property real value: targetValue
    property real velocity: 0
    property bool initialized: false
    readonly property bool running: frames.running
    signal settled
    function finish(): void {
        frames.running = false;
        value = targetValue;
        velocity = 0;
        settled();
    }
    function retarget(): void {
        if (!initialized)
            return;
        if (Theme.reducedMotion || (Math.abs(value - targetValue) <= tolerance && Math.abs(velocity) <= tolerance)) {
            finish();
            return;
        }
        frames.running = true;
    }
    function sample(seconds: real): void {
        if (!running || seconds <= 0 || !isFinite(seconds))
            return;
        const omega = Math.sqrt(spec.stiffness);
        const damping = spec.damping;
        const x = value - targetValue;
        const v = velocity;
        const decay = Math.exp(-damping * omega * seconds);
        if (damping < 1) {
            const frequency = omega * Math.sqrt(1 - damping * damping);
            const sine = Math.sin(frequency * seconds);
            const cosine = Math.cos(frequency * seconds);
            const b = (v + damping * omega * x) / frequency;
            value = targetValue + decay * (x * cosine + b * sine);
            velocity = decay * (v * cosine - (damping * omega * b + frequency * x) * sine);
        } else {
            const b = v + omega * x;
            value = targetValue + decay * (x + b * seconds);
            velocity = decay * (v - omega * b * seconds);
        }
        if (Math.abs(value - targetValue) <= tolerance && Math.abs(velocity) <= tolerance)
            finish();
    }
    onTargetValueChanged: retarget()
    Component.onCompleted: {
        value = targetValue;
        initialized = true;
    }
    property Connections preferences: Connections {
        target: Theme
        function onReducedMotionChanged(): void {
            if (Theme.reducedMotion && root.running)
                root.finish();
        }
    }
    property FrameAnimation frames: FrameAnimation {
        running: false
        onTriggered: root.sample(frameTime)
    }
}
