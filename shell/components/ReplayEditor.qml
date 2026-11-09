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
    property string operation: ""
    property var retained: []
    property bool captureAfterList: false
    readonly property bool busy: job.running || save.running
    signal settled
    spacing: 12
    function command(args): var {
        return [backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "recording", "editor", ...args];
    }
    function run(args, preserveError): void {
        if (busy)
            return;
        operation = args[0];
        if (!preserveError)
            error = "";
        job.command = command(args);
        job.running = true;
    }
    function open(): void {
        if (!session && !busy) {
            captureAfterList = captureAvailable;
            run(["list"]);
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
            if (code !== 0) {
                root.error = failure || "Replay could not be opened.";
                if (root.operation === "open")
                    Qt.callLater(() => root.run(["list"], true));
                root.captureAfterList = false;
            } else if (root.operation === "list") {
                root.retained = response.sessions ?? [];
                const capture = root.captureAfterList && root.captureAvailable && root.retained.length === 0;
                root.captureAfterList = false;
                if (capture)
                    Qt.callLater(() => root.run(["open"]));
            } else if (root.operation === "discard") {
                root.session = null;
                root.previewUrl = "";
                root.savedPath = "";
                Qt.callLater(() => root.run(["list"]));
            } else {
                root.session = response;
                if (!root.retained.some(draft => draft.id === response.id))
                    root.retained = [...root.retained,
                        {
                            id: response.id,
                            ready: true
                        }
                    ];
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
            if (code === 0 && response.saved === true)
                root.savedPath = response.path;
            else
                root.error = failure || "Clip validation failed. The source is retained.";
            root.settled();
        }
    }
    Text {
        Layout.fillWidth: true
        text: root.session ? "Replay draft · " + Math.floor(root.session.duration / 60) + ":" + String(Math.floor(root.session.duration) % 60).padStart(2, "0") + " available" : "Select footage from the last five minutes"
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
        wrapMode: Text.WordWrap
    }
    Flow {
        Layout.fillWidth: true
        spacing: 4
        ControlAction {
            objectName: "captureReplay"
            text: "Capture replay"
            compact: true
            enabled: root.captureAvailable && !root.busy && root.retained.length < 3
            onClicked: root.run(["open"])
        }
        ControlAction {
            objectName: "discardCurrentReplay"
            text: "Remove draft"
            compact: true
            enabled: !!root.session && !root.busy && !preview.running
            onClicked: root.run(["discard", root.session.id])
        }
        ControlAction {
            objectName: "browseReplayDrafts"
            text: "Replay drafts"
            compact: true
            enabled: !root.busy && !preview.running
            onClicked: {
                root.session = null;
                root.previewUrl = "";
                root.savedPath = "";
                root.run(["list"]);
            }
        }
    }
    Text {
        Layout.fillWidth: true
        visible: !root.session && root.retained.length > 0
        text: root.retained.length + " of 3 replay drafts stored. Open one or remove a draft to free space. Saved clips are kept."
        wrapMode: Text.WordWrap
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
    }
    ControlChoice {
        id: retainedChoice
        objectName: "replayDraftChoice"
        Layout.fillWidth: true
        visible: root.retained.length > 0 && !root.session
        model: root.retained.map((s, i) => ({
                    name: "Replay draft " + (i + 1) + (s.ready ? "" : " · incomplete"),
                    id: s.id
                }))
        textRole: "name"
        Accessible.name: "Replay drafts"
        enabled: !root.busy
        onActivated: index => root.run(["resume", root.retained[index].id])
    }
    ControlAction {
        Layout.fillWidth: true
        visible: root.retained.length > 0 && !root.session
        text: "Open draft"
        enabled: !root.busy
        onClicked: root.run(["resume", root.retained[Math.max(0, retainedChoice.currentIndex)].id])
    }
    ControlAction {
        Layout.fillWidth: true
        visible: root.retained.length > 0 && !root.session
        text: "Remove selected draft"
        enabled: !root.busy
        onClicked: root.run(["discard", root.retained[Math.max(0, retainedChoice.currentIndex)].id])
    }
    WidgetStatus {
        Layout.fillWidth: true
        status: root.error ? "error" : "loading"
        message: root.error || (job.running ? "Protecting footage and preparing previews…" : save.running ? "Saving and validating clip…" : !root.session && !root.retained.length && !root.captureAvailable ? "No captured replay is available. Enable Instant Replay to collect footage." : "")
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
            label: "Start"
            seconds: root.startTime
            onRequested: seconds => root.select(Math.min(seconds, root.endTime - 0.05), root.endTime)
        }
        ControlTimeField {
            Layout.fillWidth: true
            objectName: "clipEnd"
            label: "End"
            seconds: root.endTime
            onRequested: seconds => root.select(root.startTime, Math.max(seconds, root.startTime + 0.05))
        }
    }
    Text {
        visible: !!root.session
        text: (root.endTime - root.startTime).toFixed(3) + " s selected · desktop + microphone"
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
        font.features: {
            "tnum": 1
        }
    }
    ControlAction {
        Layout.fillWidth: true
        visible: !!root.session
        text: "Save selected clip"
        detail: root.savedPath ? "Saved and validated" : "MKV · separate Opus tracks"
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
