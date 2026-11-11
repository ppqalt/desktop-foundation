pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import "../theme"

// Production and gallery use the same controls. The recorder owns confirmed state.
ColumnLayout {
    id: root
    objectName: "recordingControl"
    property var recording: null
    property bool pending: false
    property int selectedMode: 0
    property alias modeChoice: audioChoice
    property alias actionButton: recordAction
    readonly property var captureState: recording?.state ?? ({})
    readonly property string phase: captureState.phase ?? "idle"
    readonly property bool compatible: ["idle", "error"].includes(phase) || captureState.control_protocol === 2
    readonly property bool startingCapture: phase === "starting" && (captureState.replay !== true || captureState.replay_paused === true)
    readonly property bool captureActive: startingCapture || ["recording", "replay-recording"].includes(phase)
    readonly property bool transitioning: ["starting", "stopping"].includes(phase)
    readonly property string actualMode: captureState.audio_mode || (captureState.silent === true ? "desktop" : "desktop-microphone")
    readonly property int actualModeIndex: actualMode === "none" ? 2 : actualMode === "desktop" ? 1 : 0
    readonly property string statusText: !recording?.prepared ? "Starting" : !compatible ? "Recorder restart needed" : phase === "error" ? "Error" : phase === "stopping" ? "Saving" : startingCapture ? "Starting" : captureActive ? "Recording" : ""
    signal requested(var requestArgs)
    spacing: 8
    Text {
        text: "Recording"
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.titleMedium
        font.weight: Font.Medium
    }
    GridLayout {
        Layout.fillWidth: true
        columns: root.width >= 320 * Material.effectiveTextScale ? 2 : 1
        columnSpacing: 8
        rowSpacing: 0
        ControlChoice {
            id: audioChoice
            flat: true
            objectName: "recordingAudioMode"
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            model: ["Desktop + microphone", "Desktop only", "No audio"]
            Accessible.name: "Recording audio"
            currentIndex: root.captureActive ? root.actualModeIndex : root.selectedMode
            enabled: !!root.recording?.prepared && root.compatible && !root.captureActive && !root.transitioning && !root.pending
            onActivated: index => root.selectedMode = index
        }
        ControlAction {
            id: recordAction
            objectName: "recordingAction"
            compact: true
            marked: root.captureActive
            Layout.alignment: Qt.AlignRight
            Layout.preferredWidth: Math.max(88, recordLabel.advanceWidth + 40)
            Layout.maximumWidth: Layout.preferredWidth
            TextMetrics {
                id: recordLabel
                font.family: Theme.typography.family
                font.pixelSize: Material.type.labelLarge
                text: recordAction.text
            }
            text: root.phase === "stopping" ? "Saving" : root.startingCapture ? "Cancel" : root.captureActive ? "Stop" : "Record"
            Accessible.name: root.captureActive ? "Stop screen recording" : "Start screen recording"
            enabled: !!root.recording?.prepared && root.compatible && !root.pending && (!root.transitioning || root.startingCapture)
            onClicked: {
                if (root.captureActive)
                    root.requested(["stop-recording"]);
                else
                    root.requested(root.selectedMode === 2 ? ["start", "--no-audio"] : root.selectedMode === 1 ? ["start", "--desktop-only"] : ["start"]);
            }
        }
    }
    Text {
        objectName: "recordingStatus"
        Layout.fillWidth: true
        visible: text.length > 0
        text: root.statusText
        color: root.phase === "error" ? Material.colors.error : Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
    }
}
