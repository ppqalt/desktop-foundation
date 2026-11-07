pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "../theme"
import "../components"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    required property var sidebarStyle
    property var recording: null
    property var targetScreen: null
    property bool handoff: false
    property bool superseded: false
    visible: !superseded
    property bool closing: false
    property string nextSurface: ""
    property string page: "overview"
    property var returnFocus: null
    readonly property var connectedDevices: (network.devices ?? []).filter(d => d.state === "connected")
    readonly property string networkSummary: !network.available ? "Unavailable" : !network.enabled ? "Off" : connectedDevices.length ? connectedDevices.map(d => d.kind === "ethernet" ? "Ethernet" : d.connection || d.name).join(" · ") : "Not connected"
    property var stateData: ({})
    property string error: ""
    property var queue: []
    property var audioQueue: []
    property bool initialFocus: true
    property int requestRevision: 0
    property int pendingOutput: -1
    property int pendingInput: -1
    property bool applicationsExpanded: false
    property var pendingApplications: ({})
    readonly property bool busy: deviceRequest.running || audioAction.running || replayRequest.running || wallpapers.busy
    readonly property var audio: stateData.audio ?? ({})
    readonly property var network: stateData.network ?? ({})
    readonly property bool wifiAvailable: (network.devices ?? []).some(d => d.kind === "wifi")
    readonly property var outputs: audio.outputs ?? []
    readonly property var inputs: audio.inputs ?? []
    readonly property var output: outputs.find(d => d.name === audio.output) ?? ({})
    readonly property var input: inputs.find(d => d.name === audio.input) ?? ({})
    readonly property string backend: Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl"
    signal dismissed(string nextSurface)
    screen: targetScreen
    implicitWidth: Math.min(sidebarStyle.width, screen?.width ?? sidebarStyle.width)
    anchors {
        top: true
        bottom: true
        left: true
    }
    // Keep one pixel mapped during entrance so the Wayland frame clock can
    // drive the first frame; closing reaches the edge and unloads the surface.
    margins.left: Math.round(-width * (1 - motion.position)) + (!closing && motion.position === 0 ? 1 : 0)
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation:control-center"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: closing || superseded ? WlrKeyboardFocus.None : WlrKeyboardFocus.OnDemand
    color: "transparent"
    Component.onCompleted: Qt.callLater(() => {
        root.present();
        root.refresh();
        outputMute.forceActiveFocus();
    })
    function navigate(destination, origin = null): void {
        if (destination !== "overview")
            returnFocus = origin;
        page = destination;
        scroll.contentY = 0;
        if (destination === "overview" && returnFocus && returnFocus.visible)
            Material.restoreFocus(returnFocus, Qt.BacktabFocusReason);
        else
            (root.page === "overview" ? networkToggle : backAction).forceActiveFocus();
    }
    function snapshot(): var {
        return {
            alive: true,
            visible: !closing,
            closing: closing,
            progress: motion.position,
            moving: motion.running,
            width: width,
            height: height,
            busy: busy,
            polling: status.running,
            replayStatus: replayControl.statusText,
            replayEnabled: replayControl.replayEnabled,
            queued: queue.length + audioQueue.length
        };
    }
    function present(): void {
        requestRevision++;
        closing = false;
        nextSurface = "";
        motion.move(true);
        refresh();
    }
    function dismiss(next = ""): void {
        if (closing) {
            if (next)
                nextSurface = next;
            return;
        }
        nextSurface = next;
        requestRevision++;
        closing = true;
        status.running = false;
        motion.move(false);
        if (superseded)
            motion.sample(1); // Hidden exits must not depend on a mapped frame clock.
    }
    function finishClose(): void {
        if (closing && !motion.running && !busy && queue.length === 0 && audioQueue.length === 0)
            dismissed(nextSurface);
    }
    function refresh(): void {
        if (!closing && !busy && queue.length === 0 && audioQueue.length === 0 && !status.running && !outputChoice.popup.visible && !inputChoice.popup.visible)
            status.running = true;
    }
    function enqueue(args: list<string>): void {
        if (closing)
            return;
        requestRevision++;
        initialFocus = false;
        const audioRequest = ["volume", "mute", "select", "stream-volume"].includes(args[0]);
        let pending = (audioRequest ? audioQueue : queue).slice();
        const existing = args[0] === "volume" || args[0] === "stream-volume" ? pending.findIndex(p => p[0] === args[0] && p[1] === args[1] && (args[0] !== "stream-volume" || p[2] === args[2])) : -1;
        if (existing >= 0)
            pending[existing] = args;
        else if (pending.length < 8)
            pending.push(args);
        if (audioRequest)
            audioQueue = pending;
        else
            queue = pending;
        error = "";
        pump();
    }
    function pumpWorker(worker, requests): var {
        if (worker.running || requests.length === 0)
            return requests;
        const pending = requests.slice();
        const args = pending.shift();
        worker.command = [backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", ...args];
        worker.running = true;
        return pending;
    }
    function pump(): void {
        audioQueue = pumpWorker(audioAction, audioQueue);
        queue = pumpWorker(deviceRequest, queue);
    }
    function completed(worker, code): void {
        requestRevision++;
        if (code !== 0 || worker.response.success === false)
            error = worker.response.error || worker.failure || "The device request failed. Try again.";
        if (worker.response.audio) {
            stateData = Object.assign({}, stateData, {
                audio: worker.response.audio
            });
            if (!audioQueue.length) {
                pendingOutput = -1;
                pendingInput = -1;
                pendingApplications = ({});
            }
        }
        if (worker === audioAction && !audioQueue.length)
            pendingApplications = ({});
        Qt.callLater(() => {
            root.pump();
            root.finishClose();
            root.refresh();
        });
    }
    function revealFocus(): void {
        const item = root.contentItem.Window.window?.activeFocusItem;
        if (!item)
            return;
        let parentItem = item;
        while (parentItem && parentItem !== body)
            parentItem = parentItem.parent;
        if (!parentItem)
            return;
        const y = item.mapToItem(body, 0, 0).y;
        if (y < scroll.contentY)
            scroll.contentY = y;
        else if (y + item.height > scroll.contentY + scroll.height)
            scroll.contentY = Math.min(scroll.contentHeight - scroll.height, y + item.height - scroll.height);
    }
    Connections {
        target: root.contentItem.Window.window
        function onActiveFocusItemChanged(): void {
            root.revealFocus();
        }
    }
    SidebarMotion {
        id: motion
        openingMs: root.sidebarStyle.openingMs
        closingMs: root.sidebarStyle.closingMs
        onSettled: open => {
            if (!open)
                root.finishClose();
        }
    }
    Timer {
        interval: 2000
        repeat: true
        running: !root.closing
        onTriggered: root.refresh()
    }
    function acceptSnapshot(response, revision): bool {
        if (revision !== requestRevision || closing || busy || queue.length || audioQueue.length)
            return false;
        if (!response.audio || !response.network)
            return false;
        stateData = response;
        if (initialFocus) {
            initialFocus = false;
            Qt.callLater(() => {
                if (!root.closing && !root.superseded && revision === root.requestRevision)
                    Material.restoreFocus(outputMute.enabled ? outputMute : root.page === "overview" ? networkToggle : backAction);
            });
        }
        if (!outputVolume.pressed)
            pendingOutput = -1;
        if (!inputVolume.pressed)
            pendingInput = -1;
        return true;
    }
    Timer {
        id: audioEventRefresh
        interval: 120
        onTriggered: root.refresh()
    }
    Process {
        id: audioEvents
        command: ["pactl", "subscribe"]
        running: !root.closing && root.applicationsExpanded
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => {
                if (data.includes("on sink-input") || data.includes("on sink #") || data.includes("on source #"))
                    audioEventRefresh.restart();
            }
        }
    }
    ControlRequest {
        id: status
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "snapshot"]
        property int revision: 0
        onRunningChanged: {
            if (running)
                revision = root.requestRevision;
        }
        onFinished: code => {
            if (revision !== root.requestRevision) {
                if (!root.closing)
                    Qt.callLater(root.refresh);
                return;
            }
            if (root.closing || root.busy || root.queue.length || root.audioQueue.length)
                return;
            if (code !== 0 || !root.acceptSnapshot(response, revision))
                root.error = "Could not refresh device state. Press Ctrl+R to retry.";
        }
    }
    ControlRequest {
        id: deviceRequest
        onFinished: code => root.completed(deviceRequest, code)
    }
    ControlRequest {
        id: audioAction
        onFinished: code => root.completed(audioAction, code)
    }
    ControlRequest {
        id: replayRequest
        expectJson: false
        onFinished: code => {
            if (code !== 0)
                root.error = failure || "Could not change Instant Replay.";
            root.finishClose();
        }
    }
    function requestReplay(action): void {
        if (closing || replayRequest.running)
            return;
        error = "";
        replayRequest.command = [Quickshell.shellDir + "/../scripts/record", action];
        replayRequest.running = true;
    }
    Rectangle {
        id: card
        anchors.fill: parent
        topLeftRadius: 0
        bottomLeftRadius: 0
        topRightRadius: root.sidebarStyle.radius
        bottomRightRadius: root.sidebarStyle.radius
        border.width: 1
        border.color: Qt.alpha(Material.colors.primary, 0.65)
        Rectangle {
            anchors.fill: parent
            anchors.margins: -1
            topLeftRadius: 0
            bottomLeftRadius: 0
            topRightRadius: root.sidebarStyle.radius + 1
            bottomRightRadius: root.sidebarStyle.radius + 1
            color: "transparent"
            border.width: 3
            border.color: Qt.alpha(Material.colors.primary, 0.10)
        }
        // The non-modal desktop frame retains its shared btop geometry/alpha.
        // M3 content cards are opaque so wallpaper never changes text contrast.
        property color baseColor: Material.colors.surface ?? "#141218"
        color: Qt.rgba(baseColor.r, baseColor.g, baseColor.b, root.sidebarStyle.opacity)
        FocusScope {
            id: focusScope
            anchors.fill: parent
            anchors.margins: root.sidebarStyle.padding
            focus: true
            Keys.onEscapePressed: event => {
                if (event.isAutoRepeat) {
                    event.accepted = true;
                    return;
                }
                if (root.page !== "overview")
                    root.navigate("overview");
                else
                    root.dismiss();
                event.accepted = true;
            }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_R && event.modifiers & Qt.ControlModifier) {
                    root.error = "";
                    root.refresh();
                    event.accepted = true;
                }
            }
            ColumnLayout {
                anchors.fill: parent
                spacing: 16
                RowLayout {
                    visible: root.page !== "overview"
                    Layout.fillWidth: true
                    Layout.minimumHeight: 48
                    spacing: 8
                    ControlAction {
                        id: backAction
                        objectName: "backAction"
                        visible: root.page !== "overview"
                        text: "Back"
                        symbol: "arrow_back"
                        iconOnly: true
                        compact: true
                        Layout.preferredWidth: 48
                        onClicked: root.navigate("overview")
                    }
                    Text {
                        visible: root.page !== "overview"
                        Layout.fillWidth: true
                        Layout.leftMargin: 12
                        text: ({
                                overview: "",
                                audio: "Audio",
                                network: "Network"
                            })[root.page]
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.headlineSmall
                        lineHeightMode: Text.FixedHeight
                        lineHeight: Material.type.headlineLine
                    }
                }
                ControlGroup {
                    Layout.fillWidth: true
                    visible: root.error !== ""
                    color: Material.colors.error_container
                    WidgetStatus {
                        Layout.fillWidth: true
                        message: root.error
                        color: Material.colors.on_error_container
                        font.pixelSize: Material.type.bodyMedium
                    }
                }
                Flickable {
                    id: scroll
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    contentWidth: width
                    contentHeight: body.height
                    boundsBehavior: Flickable.StopAtBounds
                    WheelHandler {
                        target: null
                        onWheel: event => {
                            if (!event.pixelDelta.y && !event.angleDelta.y) {
                                event.accepted = false;
                                return;
                            }
                            const delta = event.pixelDelta.y || event.angleDelta.y * 1.25;
                            scroll.contentY = Math.max(0, Math.min(scroll.contentHeight - scroll.height, scroll.contentY - delta));
                            event.accepted = true;
                        }
                    }
                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                        visible: scroll.contentHeight > scroll.height
                        padding: 0
                        contentItem: Rectangle {
                            implicitWidth: 4
                            radius: 2
                            color: Material.colors.outline
                        }
                        background: null
                    }
                    ColumnLayout {
                        id: body
                        width: scroll.width
                        height: implicitHeight
                        spacing: 16
                        ControlGroup {
                            Layout.fillWidth: true
                            visible: root.page === "overview"
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 16
                                ControlAction {
                                    id: networkToggle
                                    objectName: "networkToggle"
                                    Layout.preferredWidth: 208
                                    text: "Networking"
                                    symbol: root.wifiAvailable ? "wifi" : "lan"
                                    compact: true
                                    toggle: true
                                    marked: root.network.enabled ?? false
                                    badge: marked ? "On" : "Off"
                                    enabled: (root.network.available ?? false) && !deviceRequest.running
                                    onClicked: root.enqueue(["networking", root.network.enabled ? "off" : "on"])
                                }
                                ControlAction {
                                    Layout.fillWidth: true
                                    text: root.networkSummary
                                    detail: "Network details"
                                    badge: "›"
                                    onClicked: root.navigate("network", this)
                                }
                            }
                        }
                        GridLayout {
                            Layout.fillWidth: true
                            visible: root.page === "overview" || root.page === "audio"
                            columns: root.width >= 600 ? 2 : 1
                            rowSpacing: 16
                            columnSpacing: 16
                            ControlGroup {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                ControlAction {
                                    Layout.fillWidth: true
                                    visible: root.page === "overview"
                                    objectName: "audioDetails"
                                    text: "Output"
                                    detail: root.output.label || root.output.description || "No output available"
                                    symbol: "headphones"
                                    badge: "›"
                                    onClicked: root.navigate("audio", this)
                                }
                                ControlChoice {
                                    id: outputChoice
                                    objectName: "outputChoice"
                                    visible: root.page === "audio"
                                    Layout.fillWidth: true
                                    model: root.outputs
                                    textRole: "description"
                                    currentIndex: root.outputs.findIndex(d => d.name === root.audio.output)
                                    enabled: root.outputs.length > 0
                                    Accessible.name: "Audio output"
                                    displayText: currentIndex < 0 ? "No audio output" : currentText
                                    onActivated: index => root.enqueue(["select", "output", root.outputs[index].name])
                                }
                                RowLayout {
                                    Layout.fillWidth: true
                                    ControlAction {
                                        id: outputMute
                                        objectName: "outputMute"
                                        text: root.output.muted ? "Unmute" : "Mute"
                                        symbol: root.output.muted ? "volume_off" : "volume_up"
                                        compact: true
                                        toggle: true
                                        marked: root.output.muted ?? false
                                        enabled: root.outputs.length > 0
                                        Layout.preferredWidth: 48
                                        iconOnly: true
                                        Accessible.name: "Volume mute"
                                        onClicked: root.enqueue(["mute", "output", root.output.muted ? "off" : "on"])
                                    }
                                    ControlVolume {
                                        id: outputVolume
                                        objectName: "outputVolume"
                                        Layout.fillWidth: true
                                        enabled: root.outputs.length > 0
                                        value: root.pendingOutput >= 0 ? root.pendingOutput : root.output.volume ?? 0
                                        Accessible.name: "Output volume"
                                        onMoved: {
                                            root.pendingOutput = Math.round(value);
                                            root.enqueue(["volume", "output", String(root.pendingOutput)]);
                                        }
                                    }
                                    Text {
                                        Layout.preferredWidth: 48
                                        horizontalAlignment: Text.AlignRight
                                        text: Math.round(outputVolume.value) + "%"
                                        color: Material.colors.on_surface
                                        font.family: Theme.typography.family
                                        font.pixelSize: Material.type.bodyMedium
                                        font.features: {
                                            "tnum": 1
                                        }
                                    }
                                }
                            }
                            ControlGroup {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                ControlAction {
                                    Layout.fillWidth: true
                                    visible: root.page === "overview"
                                    objectName: "inputDetails"
                                    text: "Microphone"
                                    detail: root.input.label || root.input.description || "No microphone available"
                                    symbol: "mic"
                                    badge: "›"
                                    onClicked: root.navigate("audio", this)
                                }
                                ControlChoice {
                                    id: inputChoice
                                    objectName: "inputChoice"
                                    visible: root.page === "audio"
                                    Layout.fillWidth: true
                                    model: root.inputs
                                    textRole: "description"
                                    currentIndex: root.inputs.findIndex(d => d.name === root.audio.input)
                                    displayText: currentIndex < 0 ? "No microphone" : currentText
                                    enabled: root.inputs.length > 0
                                    Accessible.name: "Microphone device"
                                    onActivated: index => root.enqueue(["select", "input", root.inputs[index].name])
                                }
                                RowLayout {
                                    Layout.fillWidth: true
                                    ControlAction {
                                        text: root.input.muted ? "Unmute" : "Mute"
                                        symbol: root.input.muted ? "mic_off" : "mic"
                                        compact: true
                                        toggle: true
                                        marked: root.input.muted ?? false
                                        enabled: root.inputs.length > 0
                                        Layout.preferredWidth: 48
                                        iconOnly: true
                                        Accessible.name: "Microphone mute"
                                        onClicked: root.enqueue(["mute", "input", root.input.muted ? "off" : "on"])
                                    }
                                    ControlVolume {
                                        id: inputVolume
                                        objectName: "inputVolume"
                                        Layout.fillWidth: true
                                        enabled: root.inputs.length > 0
                                        value: root.pendingInput >= 0 ? root.pendingInput : root.input.volume ?? 0
                                        Accessible.name: "Microphone volume"
                                        onMoved: {
                                            root.pendingInput = Math.round(value);
                                            root.enqueue(["volume", "input", String(root.pendingInput)]);
                                        }
                                    }
                                    Text {
                                        Layout.preferredWidth: 48
                                        horizontalAlignment: Text.AlignRight
                                        text: Math.round(inputVolume.value) + "%"
                                        color: Material.colors.on_surface
                                        font.family: Theme.typography.family
                                        font.pixelSize: Material.type.bodyMedium
                                        font.features: {
                                            "tnum": 1
                                        }
                                    }
                                }
                            }
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: root.page === "overview" || root.page === "audio"
                            spacing: 0
                            ControlAction {
                                objectName: "applicationsToggle"
                                Layout.fillWidth: true
                                text: "Applications"
                                badge: root.applicationsExpanded ? "⌃" : "⌄"
                                detail: (root.audio.applications ?? []).length + " audio streams"
                                onClicked: {
                                    root.applicationsExpanded = !root.applicationsExpanded;
                                    if (root.applicationsExpanded)
                                        root.refresh();
                                }
                            }
                            Repeater {
                                model: root.applicationsExpanded ? root.audio.applications ?? [] : []
                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Text {
                                        Layout.preferredWidth: Math.min(200, parent.width * 0.35)
                                        text: parent.modelData.label || parent.modelData.name
                                        elide: Text.ElideRight
                                        color: Material.colors.on_surface
                                        font.family: Theme.typography.family
                                        font.pixelSize: Material.type.bodyMedium
                                    }
                                    ControlVolume {
                                        objectName: "applicationVolume-" + parent.modelData.serial
                                        Layout.fillWidth: true
                                        Accessible.name: parent.modelData.name + " volume"
                                        value: root.pendingApplications[parent.modelData.serial] ?? parent.modelData.volume
                                        onMoved: {
                                            const stream = parent.modelData;
                                            const values = Object.assign({}, root.pendingApplications);
                                            values[stream.serial] = Math.round(value);
                                            root.pendingApplications = values;
                                            root.enqueue(["stream-volume", String(stream.id), stream.serial, String(values[stream.serial])]);
                                        }
                                    }
                                    Text {
                                        Layout.preferredWidth: 45
                                        horizontalAlignment: Text.AlignRight
                                        text: (root.pendingApplications[parent.modelData.serial] ?? parent.modelData.volume) + "%"
                                        color: Material.colors.on_surface_variant
                                        font.family: Theme.typography.family
                                        font.pixelSize: Material.type.bodyMedium
                                    }
                                }
                            }
                        }
                        ReplayControl {
                            id: replayControl
                            Layout.fillWidth: true
                            visible: root.page === "overview"
                            recording: root.recording
                            pending: replayRequest.running
                            onSetEnabled: enabled => root.requestReplay(enabled ? "replay-on" : "replay-off")
                            onSaveRequested: root.requestReplay("replay-save")
                        }
                        ControlAction {
                            objectName: "cachyUpdateShortcut"
                            visible: root.page === "overview"
                            text: "Cachy Update"
                            symbol: "system_update_alt"
                            compact: true
                            Layout.fillWidth: true
                            onClicked: {
                                root.enqueue(["cachy-update"]);
                                root.dismiss();
                            }
                        }
                        WallpaperGrid {
                            id: wallpapers
                            Layout.fillWidth: true
                            visible: root.page === "overview"
                            active: !root.closing && visible
                            backend: root.backend
                            viewport: scroll
                            onStarted: root.error = ""
                            onFailed: message => root.error = message
                            onSettled: {
                                root.finishClose();
                                root.refresh();
                            }
                        }
                        ControlGroup {
                            Layout.fillWidth: true
                            visible: root.page === "network"
                            ControlAction {
                                Layout.fillWidth: true
                                text: "Networking"
                                symbol: "lan"
                                compact: true
                                toggle: true
                                badge: root.network.enabled ? "On" : "Off"
                                marked: root.network.enabled ?? false
                                enabled: (root.network.available ?? false) && !deviceRequest.running
                                onClicked: root.enqueue(["networking", root.network.enabled ? "off" : "on"])
                            }
                            ControlAction {
                                Layout.fillWidth: true
                                text: "Wi-Fi"
                                detail: root.wifiAvailable ? "Wireless radio" : "No Wi-Fi adapter"
                                badge: root.wifiAvailable ? root.network.wifi ? "On" : "Off" : "Unavailable"
                                marked: root.wifiAvailable && (root.network.wifi ?? false)
                                enabled: root.wifiAvailable && !deviceRequest.running
                                onClicked: root.enqueue(["wifi", root.network.wifi ? "off" : "on"])
                            }
                            Repeater {
                                model: root.network.devices ?? []
                                delegate: ControlAction {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    text: modelData.kind === "ethernet" ? "Ethernet" : modelData.kind === "wifi" ? "Wi-Fi" : modelData.name
                                    detail: modelData.name + " · " + (modelData.connection && modelData.connection !== "--" ? modelData.connection : modelData.state)
                                    badge: modelData.state === "connected" ? "Disconnect" : "Connect"
                                    marked: modelData.state === "connected"
                                    enabled: (root.network.enabled ?? false) && !deviceRequest.running
                                    onClicked: root.enqueue(["device", modelData.state === "connected" ? "disconnect" : "connect", modelData.name])
                                }
                            }
                            Repeater {
                                // Active non-VPN profiles already appear as connected devices.
                                model: (root.network.connections ?? []).filter(c => !c.active || c.kind === "vpn" || c.kind === "wireguard")
                                delegate: ControlAction {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    text: modelData.name
                                    detail: modelData.kind
                                    badge: modelData.active ? "Disconnect" : "Connect"
                                    marked: modelData.active
                                    enabled: (root.network.enabled ?? false) && !deviceRequest.running
                                    onClicked: root.enqueue(["connection", modelData.active ? "down" : "up", modelData.uuid])
                                }
                            }
                            ControlAction {
                                Layout.fillWidth: true
                                text: "Network settings"
                                detail: "Add networks or edit connections"
                                symbol: "settings"
                                badge: "›"
                                enabled: (root.network.editor ?? false) && !deviceRequest.running
                                onClicked: {
                                    root.enqueue(["network-settings"]);
                                    root.dismiss();
                                }
                            }
                        }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    Layout.leftMargin: 12
                    visible: root.busy
                    text: "Applying changes…"
                    color: Material.colors.on_surface_variant
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.labelSmall
                }
            }
        }
    }
}
