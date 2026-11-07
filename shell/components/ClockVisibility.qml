import QtQuick

// Explicit clock toggles take precedence over the recording's automatic reveal.
QtObject {
    id: root
    property bool enabled: false
    property string recordingPhase: "idle"
    property bool dismissed: false
    readonly property bool recordingActive: ["starting", "recording", "stopping", "error", "replay", "replay-recording"].includes(recordingPhase)
    readonly property bool visible: enabled || (recordingActive && !dismissed)
    onRecordingPhaseChanged: {
        if (recordingPhase === "starting" || recordingPhase === "idle")
            dismissed = false;
    }
    function toggle(): void {
        const show = !visible;
        enabled = show;
        dismissed = !show && recordingActive;
    }
}
