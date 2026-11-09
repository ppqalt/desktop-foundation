pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import "../theme"

ColumnLayout {
    id: root
    objectName: "replayControl"
    property var recording: null
    property bool pending: false
    property alias saveButton: saveAction
    readonly property var replayState: recording?.state ?? ({})
    readonly property bool replayEnabled: replayState.replay === true && replayState.phase !== "error"
    readonly property string statusText: {
        if (!recording?.prepared)
            return "Starting";
        if (replayState.phase === "error")
            return "Error";
        switch (replayState.replay_status) {
        case "starting":
            return "Starting";
        case "buffering":
            return "Buffering";
        case "ready":
            return "Ready";
        case "paused":
            return "Paused";
        default:
            return replayEnabled ? "Buffering" : "Off";
        }
    }
    readonly property int seconds: recording?.bufferSeconds ?? 0
    readonly property string duration: seconds > 0 ? Math.floor(seconds / 60) + ":" + String(seconds % 60).padStart(2, "0") : ""
    signal setEnabled(bool enabled)
    signal saveRequested
    spacing: 0
    Text {
        text: "Recording"
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.titleMedium
        font.weight: Font.Medium
    }
    RowLayout {
        Layout.fillWidth: true
        Layout.minimumWidth: root.width
        spacing: 4
        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: Math.max(0, root.width - 154)
            spacing: 0
            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: "Instant Replay"
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyLarge
            }
            Text {
                objectName: "replayStatus"
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.duration ? root.duration + " · " : "") + root.statusText + (root.replayState.save_status === "saving" ? " · Saving" : root.replayState.save_status === "saved" ? " · Saved" : root.replayState.save_status === "error" ? " · Save failed" : "")
                color: Material.colors.on_surface_variant
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
            }
        }
        ControlAction {
            id: saveAction
            objectName: "saveReplay"
            text: "Save"
            Accessible.name: "Open replay editor"
            compact: true
            Layout.preferredWidth: 82
            Layout.maximumWidth: 82
            enabled: root.replayEnabled && ["buffering", "ready"].includes(root.replayState.replay_status) && root.replayState.save_status !== "saving" && !root.pending
            onClicked: root.saveRequested()
        }
        ControlSwitch {
            objectName: "replayToggle"
            Layout.preferredWidth: 64
            Layout.maximumWidth: 64
            text: "Instant Replay"
            marked: root.replayEnabled
            pending: root.pending
            enabled: !!root.recording?.prepared && !root.pending && (root.replayEnabled || root.replayState.replay_paused === true || root.replayState.phase === "idle" || root.replayState.phase === "error")
            onClicked: root.setEnabled(!marked)
        }
    }
}
