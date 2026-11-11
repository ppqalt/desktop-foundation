import QtQuick
import QtQuick.Window
import "../theme"

// Same cubic Hermite position/velocity and reversal braking as monitor.rs.
QtObject {
    id: root
    property Item targetItem: null
    property bool awaitingFrame: false
    property int openingMs: 240
    property int closingMs: 180
    property real position: 0
    property real velocity: 0
    property real start: 0
    property real initialVelocity: 0
    property real destination: 0
    property real seconds: 0.24
    property real elapsed: 0
    property bool opening: false
    readonly property bool running: frames.running || awaitingFrame
    signal settled(bool open)
    function move(open: bool): void {
        if (awaitingFrame && !open) {
            awaitingFrame = false;
            frames.running = false;
            position = 0;
            velocity = 0;
            opening = false;
            settled(false);
            return;
        }
        const firstOpening = open && position === 0 && !frames.running && !!targetItem;
        if (Theme.reducedMotion) {
            awaitingFrame = false;
            frames.running = false;
            position = open ? 1 : 0;
            velocity = 0;
            opening = open;
            settled(open);
            return;
        }
        start = position;
        initialVelocity = velocity;
        destination = open ? 1 : 0;
        seconds = (open ? openingMs : closingMs) / 1000;
        const distance = destination - start;
        const speed = Math.abs(initialVelocity);
        if (speed > 0) {
            const room = initialVelocity > 0 ? 1 - start : start;
            if (room <= 0 || distance === 0) {
                initialVelocity = 0;
                velocity = 0;
            } else {
                const maximum = initialVelocity * distance < 0 ? 27 / 4 * room / speed : 3 * Math.abs(distance) / speed;
                seconds = Math.min(seconds, maximum);
            }
        }
        elapsed = 0;
        opening = open;
        awaitingFrame = firstOpening;
        frames.running = !awaitingFrame;
    }
    function sample(delta: real): void {
        elapsed += delta;
        const t = Math.min(1, elapsed / Math.max(seconds, 0.000001));
        if (t >= 1) {
            position = destination;
            velocity = 0;
            frames.running = false;
            settled(opening);
            return;
        }
        const distance = destination - start;
        const x = start + distance * (3 * t * t - 2 * t * t * t) + initialVelocity * seconds * (t - 2 * t * t + t * t * t);
        position = Math.max(0, Math.min(1, x));
        velocity = position === x ? distance * (6 * t - 6 * t * t) / seconds + initialVelocity * (1 - 4 * t + 3 * t * t) : 0;
    }
    property Connections preferences: Connections {
        target: Theme
        function onReducedMotionChanged(): void {
            if (Theme.reducedMotion && root.running)
                root.move(root.opening);
        }
    }
    property Connections presentation: Connections {
        target: root.targetItem ? root.targetItem.Window.window : null
        enabled: root.awaitingFrame
        function onFrameSwapped(): void {
            root.awaitingFrame = false;
            root.frames.running = true;
        }
    }
    property FrameAnimation frames: FrameAnimation {
        running: false
        onTriggered: root.sample(frameTime)
    }
}
