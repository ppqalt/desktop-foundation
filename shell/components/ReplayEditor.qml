pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import "../theme"

ColumnLayout {
    id: root
    objectName: "replayEditor"
    required property string backend
    property bool captureAvailable: true
    property var session: null
    property real startTime: 0
    property real endTime: 0
    property real playhead: 0
    property string previewUrl: ""
    property string error: ""
    property string savedPath: ""
    readonly property bool busy: job.running || save.running || cleanup.running
    signal settled
    spacing: 12
    function command(args): var {
        return [backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "recording", "editor", ...args];
    }
    function run(args): void {
        if (busy)
            return;
        error = "";
        job.command = command(args);
        job.running = true;
    }
    function open(): void {
        if (!busy) {
            savedPath = "";
            run(["capture"]);
        }
    }
    function select(start: real, end: real): void {
        if (!session)
            return;
        startTime = Math.max(0, Math.min(start, end - 0.05));
        endTime = Math.min(session.duration, Math.max(end, startTime + 0.05));
        savedPath = "";
    }
    function scrub(seconds: real): void {
        if (!session)
            return;
        playhead = Math.max(0, Math.min(session.duration, seconds));
        previewTimer.restart();
    }
    function loadPreview(): void {
        if (session && !preview.running && !busy) {
            preview.command = command(["preview", session.id, String(playhead)]);
            preview.running = true;
        }
    }
    ControlRequest {
        id: job
        onFinished: code => {
            if (code !== 0)
                root.error = failure || "Replay could not be opened.";
            else {
                root.session = response;
                root.error = response.export_error || "";
                root.startTime = Math.max(0, response.duration - 47);
                root.endTime = response.duration;
                root.playhead = root.startTime;
                root.savedPath = "";
                root.scrub(root.playhead);
            }
            root.settled();
        }
    }
    ControlRequest {
        id: preview
        property real requestedTime: 0
        property string requestedSession: ""
        onStarted: {
            requestedTime = root.playhead;
            requestedSession = root.session?.id ?? "";
        }
        onFinished: code => {
            const current = root.session?.id === requestedSession;
            if (code !== 0 && current)
                root.error = failure || "Frame preview failed; the source is retained.";
            if (code === 0 && current && Math.abs(requestedTime - root.playhead) < 0.001)
                root.previewUrl = response.url;
            if (!current || Math.abs(requestedTime - root.playhead) >= 0.001)
                previewTimer.restart();
        }
    }
    Timer {
        id: previewTimer
        interval: 120
        onTriggered: root.loadPreview()
    }
    ControlRequest {
        id: save
        onFinished: code => {
            if (code === 0 && response.saved === true) {
                root.savedPath = response.path;
                cleanup.command = root.command(["discard", root.session.id]);
                cleanup.running = true;
            } else
                root.error = failure || "Clip validation failed. Your footage is still available.";
            root.settled();
        }
    }
    ControlRequest {
        id: cleanup
        onFinished: code => {
            if (code !== 0)
                root.error = "Clip saved, but temporary footage could not be cleaned up.";
            root.session = null;
            root.previewUrl = "";
            root.settled();
        }
    }
    Text {
        Layout.fillWidth: true
        text: root.savedPath ? "Clip saved." : root.session ? "Footage · " + Math.floor(root.session.duration / 60) + ":" + String(Math.floor(root.session.duration) % 60).padStart(2, "0") + " available" : "Select footage from the last five minutes"
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
        wrapMode: Text.WordWrap
    }
    WidgetStatus {
        Layout.fillWidth: true
        status: root.error ? "error" : "loading"
        message: root.error || (job.running ? "Preparing replay…" : save.running ? "Saving and validating clip…" : !root.session && !root.savedPath && !root.captureAvailable ? "No captured replay is available. Enable Instant Replay to collect footage." : "")
    }
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: Math.min(260, width * 9 / 16)
        visible: !!root.session
        color: Material.colors.surface_container_lowest
        radius: Material.shape.medium
        Image {
            anchors.fill: parent
            source: root.previewUrl
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            sourceSize.width: 640
            sourceSize.height: 360
        }
        Text {
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 8
            text: startField.format(root.playhead) + (preview.running ? " …" : "")
            color: Material.colors.on_primary_container
            font.family: Theme.typography.family
            font.pixelSize: Material.type.labelSmall
            font.features: {
                "tnum": 1
            }
            Rectangle {
                anchors.fill: parent
                anchors.margins: -4
                z: -1
                radius: 4
                color: Material.colors.primary_container
            }
        }
        Text {
            anchors.centerIn: parent
            visible: !root.previewUrl
            text: "Loading frame…"
            color: Material.colors.on_surface_variant
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
        }
    }
    ReplayTimeline {
        Layout.fillWidth: true
        visible: !!root.session
        enabled: !save.running
        thumbnails: root.session?.thumbnails ?? []
        duration: root.session?.duration ?? 0
        startTime: root.startTime
        endTime: root.endTime
        playhead: root.playhead
        frameStep: 1 / (root.session?.fps ?? 60)
        onSelectionRequested: (start, end) => root.select(start, end)
        onScrubRequested: seconds => root.scrub(seconds)
    }
    RowLayout {
        Layout.fillWidth: true
        visible: !!root.session
        enabled: !root.busy
        ControlTimeField {
            id: startField
            objectName: "clipStart"
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            Layout.minimumWidth: 0
            label: "Start"
            seconds: root.startTime
            onRequested: seconds => root.select(Math.min(seconds, root.endTime - 0.05), root.endTime)
        }
        ControlTimeField {
            Layout.fillWidth: true
            objectName: "clipEnd"
            Layout.preferredWidth: 1
            Layout.minimumWidth: 0
            label: "End"
            seconds: root.endTime
            onRequested: seconds => root.select(root.startTime, Math.max(seconds, root.startTime + 0.05))
        }
    }
    Text {
        visible: !!root.session
        text: (root.endTime - root.startTime).toFixed(2) + " s selected · desktop + microphone"
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
        font.features: {
            "tnum": 1
        }
    }
    ControlAction {
        Layout.alignment: Qt.AlignRight
        visible: !!root.session
        objectName: "saveReplayClip"
        text: save.running ? "Saving…" : "Save clip"
        compact: true
        marked: true
        enabled: !root.busy && !preview.running && root.endTime - root.startTime >= 0.05
        onClicked: {
            root.error = "";
            root.savedPath = "";
            save.command = root.command(["save", root.session.id, String(root.startTime), String(root.endTime)]);
            save.running = true;
        }
    }
    Text {
        visible: root.savedPath !== ""
        Layout.fillWidth: true
        text: root.savedPath
        textFormat: Text.PlainText
        wrapMode: Text.WrapAnywhere
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.labelSmall
    }
}
