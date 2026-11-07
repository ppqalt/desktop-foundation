pragma ComponentBehavior: Bound
import QtQuick
import "../components"
import Quickshell
import Quickshell.Wayland
import "../theme"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    property var targetScreen: null
    property bool entered: false
    property string recordingPhase: "idle"
    property bool replayActive: false
    readonly property bool recordingVisible: ["starting", "recording", "stopping", "error", "replay-recording"].includes(recordingPhase)
    screen: targetScreen
    visible: true
    implicitWidth: 76 + (recordingVisible ? 18 : 0) + (replayActive ? 14 : 0)
    implicitHeight: 34
    anchors {
        left: true
        top: true
    }
    margins.left: 24
    margins.top: 24
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-clock"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    color: "transparent"
    mask: Region {}
    Component.onCompleted: entered = true

    // The owner destroys this surface while hidden, including its minute timer.
    SystemClock {
        id: clock
        precision: SystemClock.Minutes
        enabled: root.visible
    }
    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: Material.colors.surface_container_low
        border.width: 1
        border.color: Qt.alpha(Material.colors.primary, 0.65)
        SurfaceOutline {
            cornerRadius: parent.radius
        }
        opacity: root.entered ? 1 : 0
        Behavior on opacity {
            SurfaceAnimation {
                duration: Theme.timing.fast
                easing.type: Theme.easing
            }
        }
        Row {
            anchors.centerIn: parent
            spacing: 8
            Rectangle {
                visible: root.recordingVisible
                width: 8
                height: 8
                radius: 4
                anchors.verticalCenter: parent.verticalCenter
                color: root.recordingPhase === "starting" ? Material.colors.primary : Material.colors.error
                Accessible.name: ["recording", "replay-recording"].includes(root.recordingPhase) ? "Recording" : root.recordingPhase === "starting" ? "Preparing recording" : root.recordingPhase === "error" ? "Recording failed; inspect recording status" : "Saving recording"
                Accessible.role: Accessible.Indicator
                Text {
                    anchors.centerIn: parent
                    visible: root.recordingPhase === "error"
                    text: "!"
                    font.pixelSize: 8
                    font.bold: true
                    color: Material.colors.on_error
                }
            }
            Rectangle {
                visible: root.replayActive
                width: 8
                height: 8
                radius: 4
                anchors.verticalCenter: parent.verticalCenter
                color: "transparent"
                border.width: 2
                border.color: Material.colors.primary
                Accessible.name: "Replay buffer active"
                Accessible.role: Accessible.Indicator
            }
            Text {
                text: Qt.formatDateTime(clock.date, "hh:mm")
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.labelLarge
            }
        }
    }
}
