import QtQuick
import "../theme"

// One staged page replacement. Callers commit content only at opacity zero.
// The latest destination wins, and returning to the current page reverses exit.
QtObject {
    id: root
    property string initialKey: "overview"
    property string currentKey: initialKey
    property string requestedKey: currentKey
    property string phase: "idle"
    property real opacity: 1
    property real offset: 0
    property int direction: 1
    readonly property bool running: phase !== "idle"
    readonly property bool acceptsInput: phase !== "leaving"
    signal committed(string key)
    signal cancelled
    signal settled
    function animate(entering): void {
        slide.stop();
        fade.stop();
        const duration = entering ? Theme.timing.normal : Theme.timing.pageExit;
        phase = entering ? "entering" : "leaving";
        slide.duration = duration;
        fade.duration = duration;
        slide.from = offset;
        slide.to = entering ? 0 : -direction * Material.motionDistance.pageExit;
        fade.from = opacity;
        fade.to = entering ? 1 : 0;
        slide.start();
        fade.start();
    }
    function request(key, forward = true): void {
        requestedKey = key;
        const nextDirection = forward ? 1 : -1;
        const changedDirection = nextDirection !== direction;
        direction = nextDirection;
        if (key === currentKey) {
            if (phase === "leaving") {
                animate(true);
                cancelled();
            }
        } else if (phase !== "leaving" || changedDirection) {
            animate(false);
        }
    }
    property SurfaceAnimation slide: SurfaceAnimation {
        target: root
        property: "offset"
    }
    property SurfaceAnimation fade: SurfaceAnimation {
        target: root
        property: "opacity"
        onFinished: {
            if (root.phase === "leaving") {
                root.slide.complete();
                const destination = root.requestedKey;
                root.offset = root.direction * Material.motionDistance.pageEnter;
                root.phase = "entering";
                root.currentKey = destination;
                root.committed(destination);
                // A callback may already have requested another destination or
                // completed a zero-duration transition. Never overwrite it.
                if (root.phase === "entering" && root.currentKey === destination && root.requestedKey === destination)
                    root.animate(true);
            } else if (root.phase === "entering") {
                root.slide.complete();
                root.phase = "idle";
                root.settled();
            }
        }
    }
    property Connections preferences: Connections {
        target: Theme
        function onReducedMotionChanged(): void {
            if (Theme.reducedMotion && root.running) {
                root.slide.complete();
                root.fade.complete();
            }
        }
    }
}
