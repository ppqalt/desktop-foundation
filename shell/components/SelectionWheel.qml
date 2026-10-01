pragma ComponentBehavior: Bound
import QtQuick

WheelHandler {
    id: root
    target: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    property real accumulated: 0
    signal stepped(int delta)
    onWheel: event => {
        const angle = event.angleDelta.y;
        const delta = angle !== 0 ? angle : event.pixelDelta.y;
        const threshold = angle !== 0 ? 120 : 40;
        if (delta === 0)
            return;
        if (accumulated * delta < 0)
            accumulated = 0;
        accumulated += delta;
        const steps = Math.trunc(accumulated / threshold);
        if (steps !== 0) {
            accumulated -= steps * threshold;
            stepped(-steps);
        }
        event.accepted = true;
    }
}
