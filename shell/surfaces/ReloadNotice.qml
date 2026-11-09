pragma ComponentBehavior: Bound
import QtQuick
import Quickshell

// Successful development reloads are silent. Failures stay visible without
// taking input; the explicit black surface is the owner's requested exception.
// https://quickshell.org/docs/v0.2.0/types/Quickshell/Quickshell/#method-inhibitReloadPopup
Scope {
    id: root
    property string message: ""
    property bool shown: false
    property var targetScreen: Quickshell.screens[0] ?? null
    function present(error: string): void {
        message = error;
        shown = true;
        expiry.restart();
    }
    Connections {
        target: Quickshell
        function onReloadCompleted(): void {
            Quickshell.inhibitReloadPopup();
        }
        function onReloadFailed(errorString: string): void {
            Quickshell.inhibitReloadPopup();
            root.present(errorString);
        }
    }
    Timer {
        id: expiry
        interval: 8000
        onTriggered: root.shown = false
    }
    LazyLoader {
        active: root.shown
        source: Quickshell.env("WAYLAND_DISPLAY") ? Qt.resolvedUrl("ReloadFailure.qml") : ""
        onItemChanged: {
            if (item) {
                item.message = root.message;
                item.targetScreen = root.targetScreen;
            }
        }
    }
}
