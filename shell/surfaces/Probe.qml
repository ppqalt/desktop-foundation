import QtQuick
import Quickshell
import Quickshell.Wayland
import "../components"
import "../theme"

// qmllint disable uncreatable-type
// Quickshell makes this platform-selected type creatable during application startup.
PanelWindow {
    id: probe
    required property var compositor
    required property var lifecycle
    implicitWidth: Theme.dimensions.probeWidth
    implicitHeight: Theme.dimensions.probeHeight
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-probe"
    WlrLayershell.layer: WlrLayer.Overlay
    color: Theme.testBackground
    DiagnosticText {
        anchors.fill: parent
        text: "Temporary foundation probe\n" + (probe.compositor.activeWorkspace ? "Workspace: " + probe.compositor.activeWorkspace.name : "Connecting…") + "\n" + (probe.compositor.focusedWindow ? probe.compositor.focusedWindow.title : "No focused window")
    }
    Component.onCompleted: {
        lifecycle.probeAlive = true;
        lifecycle.probeCreations++;
    }
    Component.onDestruction: {
        lifecycle.probeAlive = false;
        lifecycle.probeDestructions++;
    }
}
