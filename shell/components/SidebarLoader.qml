import QtQuick
import Quickshell

// Prepare between frames, retain a hidden sidebar, and never force incubation
// through an early item read. The surface must support startHidden/present/dismiss.
LazyLoader {
    id: root
    property bool requestedOpen: false
    readonly property var surface: active ? item : null
    Component.onCompleted: loading = true
    onActiveChanged: {
        if (active && requestedOpen)
            item.present();
    }
    function present(): void {
        requestedOpen = true;
        loading = true;
        if (active)
            item.present();
    }
    function dismiss(): void {
        requestedOpen = false;
        if (active)
            item.dismiss();
    }
}
