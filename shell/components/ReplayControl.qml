pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import "../theme"

ColumnLayout {
    id: root
    objectName: "replayControl"
    property var recording: null
    property bool pending: false
    property bool showHeading: true
    readonly property bool stacked: width < 320 * Material.effectiveTextScale
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
        visible: root.showHeading
        text: "Recording"
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.titleMedium
        font.weight: Font.Medium
    }
    Item {
        Layout.fillWidth: true
        implicitHeight: root.stacked ? replayLabel.implicitHeight + replayActions.implicitHeight : Math.max(replayLabel.implicitHeight, replayActions.implicitHeight)
        ColumnLayout {
            id: replayLabel
            anchors.left: parent.left
            y: root.stacked ? 0 : (parent.height - height) / 2
            width: root.stacked ? parent.width : Math.max(0, parent.width - replayActions.width - 4)
            height: implicitHeight
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
        RowLayout {
            id: replayActions
            anchors.right: parent.right
            y: root.stacked ? replayLabel.implicitHeight : 0
            spacing: 4
            ControlAction {
                id: saveAction
                objectName: "saveReplay"
                text: "Save"
                Accessible.name: "Open replay editor"
                compact: true
                Layout.preferredWidth: Math.max(82, minimumContentWidth)
                Layout.maximumWidth: Layout.preferredWidth
                enabled: !!root.recording?.prepared && root.replayState.replay === true && ["buffering", "ready"].includes(root.replayState.replay_status) && root.replayState.phase !== "error" && root.replayState.save_status !== "saving" && !root.pending
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
    WidgetStatus {
        objectName: "replayFeedback"
        Layout.fillWidth: true
        Layout.topMargin: 8
        status: root.replayState.phase === "error" || root.replayState.save_status === "error" ? "error" : "loading"
        message: status === "error" ? root.replayState.save_error || root.replayState.error || "Recording failed" : root.replayState.replay_status === "paused" ? "Paused · " + (root.replayState.audio_mode === "none" ? "video-only" : "desktop-only") + " recording; buffer resets." : ""
        font.pixelSize: Material.type.bodyMedium
    }
}
