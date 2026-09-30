import QtQuick
import Quickshell
import Quickshell.Wayland
import "../components"
import "../theme"
PanelWindow {
    required property var compositor
    implicitWidth: 440
    implicitHeight: 100
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-probe"
    WlrLayershell.layer: WlrLayer.Overlay
    color: Theme.testBackground
    DiagnosticText {
        anchors.fill: parent
        text: "Temporary foundation probe\n" + (compositor.activeWorkspace ? "Workspace: " + compositor.activeWorkspace.name : "Connecting…") + "\n" + (compositor.focusedWindow ? compositor.focusedWindow.title : "No focused window")
    }
    Component.onCompleted: console.info("PROBE_CREATED")
    Component.onDestruction: console.info("PROBE_DESTROYED")
}
