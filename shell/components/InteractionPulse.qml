import QtQuick
import "../theme"

// One short activation latch for both state layers and press geometry.
QtObject {
    id: root
    property bool pressed: false
    property bool enabled: true
    readonly property bool active: clock.running
    function trigger(): void {
        if (enabled && !Theme.reducedMotion)
            clock.restart();
    }
    onPressedChanged: {
        if (pressed)
            trigger();
    }
    onEnabledChanged: {
        if (!enabled)
            clock.stop();
    }
    property Timer clock: Timer {
        interval: Theme.timing.activation
    }
    property Connections preferences: Connections {
        target: Theme
        function onReducedMotionChanged(): void {
            if (Theme.reducedMotion)
                root.clock.stop();
        }
    }
}
