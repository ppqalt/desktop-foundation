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
    implicitWidth: 440
    implicitHeight: 100
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-probe"
    WlrLayershell.layer: WlrLayer.Overlay
    color: Theme.testBackground
    DiagnosticText {
        anchors.fill: parent
        text: "Temporary foundation probe\n" + (probe.compositor.activeWorkspace ? "Workspace: " + probe.compositor.activeWorkspace.name : "Connecting…") + "\n" + (probe.compositor.focusedWindow ? probe.compositor.focusedWindow.title : "No focused window")
    }
    Component.onCompleted: console.info("PROBE_CREATED")
    Component.onDestruction: console.info("PROBE_DESTROYED")
}
