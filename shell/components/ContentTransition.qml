import QtQuick
import "../theme"

// One staged page replacement. Callers commit content only at opacity zero.
// The latest destination wins, and returning to the current page reverses exit.
QtObject {
    id: root
    property string currentKey: "overview"
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
        phase = entering ? "entering" : "leaving";
        slide.from = offset;
        slide.to = entering ? 0 : -direction * Material.motionDistance.pageExit;
        fade.from = opacity;
        fade.to = entering ? 1 : 0;
        slide.start();
        fade.start();
    }
    function request(key, forward = true): void {
        requestedKey = key;
        direction = forward ? 1 : -1;
        if (key === currentKey) {
            if (phase === "leaving") {
                animate(true);
                cancelled();
            }
        } else if (phase !== "leaving") {
            animate(false);
        }
    }
    property SurfaceAnimation slide: SurfaceAnimation {
        target: root
        property: "offset"
        duration: root.phase === "leaving" ? Theme.timing.pageExit : Theme.timing.normal
    }
    property SurfaceAnimation fade: SurfaceAnimation {
        target: root
        property: "opacity"
        duration: root.slide.duration
        onFinished: {
            if (root.phase === "leaving") {
                root.slide.complete();
                root.currentKey = root.requestedKey;
                root.offset = root.direction * Material.motionDistance.pageEnter;
                root.phase = "entering";
                root.committed(root.currentKey);
                root.animate(true);
            } else if (root.phase === "entering") {
                root.slide.complete();
                root.phase = "idle";
                root.settled();
            }
        }
    }
}
