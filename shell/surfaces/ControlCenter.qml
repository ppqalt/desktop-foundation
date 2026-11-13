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
    property bool closeReported: false
    property bool contentReady: false
    visible: !superseded
    property bool closing: false
    property string nextSurface: ""
    property string page: "overview"
    property var returnFocus: null
    property int pageFocusReason: Qt.OtherFocusReason
    readonly property var connectedDevices: (network.devices ?? []).filter(d => d.state === "connected")
    readonly property string networkSummary: !network.available ? "Unavailable" : !network.enabled ? "Off" : connectedDevices.length ? connectedDevices.map(d => d.kind === "ethernet" ? "Ethernet" : d.connection || d.name).join(" · ") : "Not connected"
    property var stateData: ({})
    property var rememberedState: null
    property bool receivedSnapshot: false
    property bool audioFresh: false
    property bool networkFresh: false
    readonly property bool refreshingRemembered: Object.keys(stateData).length > 0 && (!receivedSnapshot || ((!audioFresh || !networkFresh) && status.running))
    function applyRemembered(): void {
        if (closing || superseded || !rememberedState)
            return;
        if (rememberedState.live) {
            applyLive();
            return;
        }
        const value = rememberedState.snapshot;
        const next = Object.assign({}, stateData);
        for (const key of ["audio", "network"]) {
            const fresh = key === "audio" ? audioFresh : networkFresh;
            if (!fresh && value[key]?.available === true)
                next[key] = receivedSnapshot ? Object.assign({}, value[key], stateData[key]) : value[key];
        }
        stateData = next;
    }
    function applyLive(): void {
        if (closing || superseded || !rememberedState?.live)
            return;
        const value = rememberedState.current;
        const next = Object.assign({}, stateData);
        if (value.audio && (value.audio.available !== true || (!audioAction.running && !audioQueue.length)))
            next.audio = value.audio;
        if (value.network && (value.network.available !== true || (!deviceRequest.running && !queue.length)))
            next.network = value.network;
        stateData = next;
        syncApplications();
        audioFresh = next.audio?.available === true;
        networkFresh = next.network?.available === true;
        receivedSnapshot = !!value.audio && !!value.network;
        refreshError = [value.audio?.available === false ? "Audio unavailable" : "", value.network?.available === false ? "Network unavailable" : ""].filter(Boolean).join(" · ");
    }
    Connections {
        target: root.rememberedState
        function onCurrentChanged(): void {
            root.applyLive();
        }
        function onSnapshotChanged(): void {
            root.applyRemembered();
        }
    }
    property string error: ""
    property string refreshError: ""
    property var queue: []
    property var audioQueue: []
    property bool initialFocus: true
    property int requestRevision: 0
    property int pendingOutput: -1
    property int pendingInput: -1
    property var pendingApplications: ({})
    readonly property bool busy: deviceRequest.running || audioAction.running || replayRequest.running || replayEditor.busy || wallpapers.busy || cachyUpdate.busy
    readonly property var audio: stateData.audio ?? ({})
    // Keep delegates keyed by stream serial. Replacing a JS array model on
    // every audio response destroys the pressed slider and its pointer grab.
    ListModel {
        id: applicationModel
    }
    onAudioChanged: Qt.callLater(root.syncApplications)
    function syncApplications(): void {
        const rows = root.audio.applications ?? [];
        for (let index = 0; index < rows.length; index++) {
            const stream = rows[index];
            const serial = String(stream.serial);
            let existing = index;
            while (existing < applicationModel.count && applicationModel.get(existing).streamSerial !== serial)
                existing++;
            const row = {
                streamSerial: serial,
                streamId: Number(stream.id),
                streamName: String(stream.name),
                streamLabel: String(stream.label || stream.name),
                streamVolume: Number(stream.volume)
            };
            if (existing === applicationModel.count)
                applicationModel.insert(index, row);
            else {
                if (existing !== index)
                    applicationModel.move(existing, index, 1);
                applicationModel.set(index, row);
            }
        }
        if (applicationModel.count > rows.length)
            applicationModel.remove(rows.length, applicationModel.count - rows.length);
    }
    readonly property var network: stateData.network ?? ({})
    readonly property bool wifiAvailable: (network.devices ?? []).some(d => d.kind === "wifi")
    readonly property var outputs: audio.outputs ?? []
    readonly property var inputs: audio.inputs ?? []
    readonly property var output: outputs.find(d => d.name === audio.output) ?? ({})
    readonly property var input: inputs.find(d => d.name === audio.input) ?? ({})
    readonly property var outputMute: outputLevel.muteButton
    readonly property var outputVolume: outputLevel.slider
    readonly property var inputVolume: inputLevel.slider
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
    Component.onCompleted: {
        // Project the shell's current values before the window's first frame.
        root.applyRemembered();
        Qt.callLater(() => {
            if (root.closing || root.superseded)
                return;
            root.applyRemembered();
            root.rememberedState?.restore();
            root.present();
        });
    }
    function pageEntryControl(): var {
        return root.page === "overview" ? (networkToggle.enabled ? networkToggle : audioDetails) : backAction;
    }
    function navigate(destination, origin = null): void {
        initialFocus = false;
        const active = root.contentItem.Window.window?.activeFocusItem;
        pageFocusReason = origin?.focusReason ?? active?.focusReason ?? Qt.OtherFocusReason;
        if (destination !== "overview")
            returnFocus = origin;
        pageMotion.request(destination, destination !== "overview");
    }
    function commitPage(destination): void {
        root.page = destination;
        scroll.contentY = 0;
        if (root.closing || root.superseded)
            return;
        if (destination === "overview" && returnFocus && returnFocus.visible && returnFocus.enabled)
            Material.restoreFocus(returnFocus, Qt.BacktabFocusReason);
        else
            Material.restoreFocus(root.pageEntryControl(), pageFocusReason);
    }
    function snapshot(): var {
        return {
            alive: true,
            visible: !closing,
            closing: closing,
            progress: motion.position,
            contentReady: contentReady,
            moving: motion.running,
            audioFresh: audioFresh,
            networkFresh: networkFresh,
            refreshingRemembered: refreshingRemembered,
            pageMoving: pageMotion.running,
            page: root.page,
            requestedPage: pageMotion.requestedKey,
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
        closeReported = false;
        initialFocus = true;
        closing = false;
        contentReady = false;
        nextSurface = "";
        motion.move(true);
        // Keyboard dismissal must not wait for device discovery on first open.
        Qt.callLater(() => {
            if (!root.closing && !root.superseded) {
                if (outputMute.enabled)
                    Material.restoreFocus(outputMute);
                else
                    focusScope.forceActiveFocus();
            }
        });
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
        contentReady = false;
        status.running = false;
        motion.move(false);
        if (superseded)
            motion.sample(1); // Hidden exits must not depend on a mapped frame clock.
    }
    function finishClose(): void {
        if (closing && !closeReported && !motion.running && !busy && queue.length === 0 && audioQueue.length === 0) {
            closeReported = true;
            dismissed(nextSurface);
        }
    }
    function refresh(force = false): void {
        if (rememberedState?.live) {
            if (force)
                rememberedState.refreshLive();
            return;
        }
        if (!closing && contentReady && !motion.running && !busy && queue.length === 0 && audioQueue.length === 0 && !status.running && !outputChoice.popup.visible && !inputChoice.popup.visible)
            status.running = true;
    }
    function enqueue(args: list<string>): void {
        if (closing)
            return;
        const audioRequest = ["volume", "mute", "select", "stream-volume"].includes(args[0]);
        if (audioRequest ? !audioFresh : !networkFresh)
            return;
        if (rememberedState?.live)
            rememberedState.beginAction(audioRequest ? "audio" : "network");
        requestRevision++;
        initialFocus = false;
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
            audioFresh = worker.response.audio.available === true;
            stateData = Object.assign({}, stateData, {
                audio: worker.response.audio
            });
            if (!audioQueue.length) {
                pendingOutput = -1;
                pendingInput = -1;
                pendingApplications = ({});
            }
        }
        if (worker.response.network) {
            networkFresh = worker.response.network.available === true;
            stateData = Object.assign({}, stateData, {
                network: worker.response.network
            });
        }
        if (rememberedState?.live)
            rememberedState.confirm(worker.response);
        else
            rememberedState?.remember(worker.response);
        if (worker === audioAction && !audioQueue.length)
            pendingApplications = ({});
        Qt.callLater(() => {
            root.pump();
            root.finishClose();
            if (!worker.response.audio && !worker.response.network)
                root.refresh(true);
        });
    }
    FocusViewport {
        viewport: scroll
    }
    Connections {
        target: root.contentItem.Window.window
        function onActiveFocusItemChanged(): void {
            const active = root.contentItem.Window.window?.activeFocusItem;
            if (active?.activeFocusOnTab)
                root.initialFocus = false;
        }
    }
    SidebarMotion {
        id: motion
        targetItem: root.contentItem
        openingMs: root.sidebarStyle.openingMs
        closingMs: root.sidebarStyle.closingMs
        onSettled: open => {
            if (!open)
                root.finishClose();
            else {
                root.contentReady = true;
                root.refresh();
            }
        }
    }
    Timer {
        interval: 2000
        repeat: true
        running: !root.closing && !root.rememberedState?.live
        onTriggered: root.refresh()
    }
    function acceptSnapshot(response, revision): bool {
        if (revision !== requestRevision || closing || busy || queue.length || audioQueue.length)
            return false;
        if (!response.audio || !response.network)
            return false;
        receivedSnapshot = true;
        const audioAvailable = response.audio.available === true;
        const networkAvailable = response.network.available === true;
        stateData = {
            audio: audioAvailable ? response.audio : Object.assign({}, stateData.audio, response.audio),
            network: networkAvailable ? response.network : Object.assign({}, stateData.network, response.network)
        };
        // Retire cached stream identities before enabling their controls.
        syncApplications();
        audioFresh = audioAvailable;
        networkFresh = networkAvailable;
        rememberedState?.remember(response);
        refreshError = [audioFresh ? "" : "Audio unavailable", networkFresh ? "" : "Network unavailable"].filter(Boolean).join(" · ");
        if (initialFocus) {
            initialFocus = false;
            Qt.callLater(() => {
                if (!root.closing && !root.superseded && revision === root.requestRevision)
                    Material.restoreFocus(outputMute.enabled ? outputMute : root.pageEntryControl());
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
        running: !root.closing && root.page === "audio" && !root.rememberedState?.live
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
                root.refreshError = "Could not refresh device state. Press Ctrl+R to retry.";
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
                root.error = failure || "Could not change recording.";
            root.finishClose();
        }
    }
    function requestReplay(action): void {
        if (closing || replayRequest.running)
            return;
        error = "";
        replayRequest.command = [Quickshell.shellDir + "/../scripts/record"].concat(Array.isArray(action) ? action : [action]);
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
            objectName: "controlCenterFocusScope"
            anchors.fill: parent
            anchors.margins: root.sidebarStyle.padding
            focus: true
            Keys.onEscapePressed: event => {
                if (event.isAutoRepeat) {
                    event.accepted = true;
                    return;
                }
                if (root.page !== "overview" || pageMotion.requestedKey !== "overview")
                    root.navigate("overview");
                else
                    root.dismiss();
                event.accepted = true;
            }
            // A first-open snapshot may leave focus on this scope while devices
            // are unavailable. Navigation must enter usable controls immediately.
            Keys.onPressed: event => {
                const active = root.contentItem.Window.window?.activeFocusItem;
                const direction = Material.direction(event.key);
                if (direction && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) && pageMotion.acceptsInput && (!active || active === focusScope || active === pageMotion)) {
                    root.initialFocus = false;
                    Material.restoreFocus(root.pageEntryControl(), Qt.TabFocusReason);
                    event.accepted = true;
                    return;
                }
                if (event.key === Qt.Key_R && event.modifiers & Qt.ControlModifier) {
                    root.error = "";
                    root.refreshError = "";
                    root.refresh(true);
                    event.accepted = true;
                }
            }
            PagePane {
                id: pageMotion
                objectName: "controlCenterPageMotion"
                anchors.fill: parent
                focusAllowed: !root.closing && !root.superseded
                onCommitted: destination => root.commitPage(destination)
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
                                    network: "Network",
                                    replay: "Save Replay"
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
                        visible: root.error !== "" || root.refreshError !== ""
                        color: Material.colors.error_container
                        WidgetStatus {
                            Layout.fillWidth: true
                            message: root.error || root.refreshError
                            status: "error"
                            color: root.error || root.refreshError ? Material.colors.on_error_container : Material.colors.on_surface_variant
                            font.pixelSize: Material.type.bodyMedium
                        }
                    }
                    Flickable {
                        id: scroll
                        objectName: "controlCenterScroll"
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
                                        badge: root.rememberedState?.live && !root.networkFresh ? "—" : marked ? "On" : "Off"
                                        enabled: root.networkFresh && (root.network.available ?? false) && !deviceRequest.running
                                        onClicked: root.enqueue(["networking", root.network.enabled ? "off" : "on"])
                                    }
                                    ControlAction {
                                        objectName: "networkDetails"
                                        Layout.fillWidth: true
                                        text: root.networkSummary
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
                                    Text {
                                        visible: root.page === "audio"
                                        text: "Output"
                                        color: Material.colors.on_surface
                                        font.family: Theme.typography.family
                                        font.pixelSize: Material.type.titleMedium
                                    }
                                    ControlAction {
                                        id: audioDetails
                                        Layout.fillWidth: true
                                        visible: root.page === "overview"
                                        objectName: "audioDetails"
                                        text: "Output"
                                        detail: root.rememberedState?.live && !root.audioFresh ? "" : root.output.label || root.output.description || "No output available"
                                        symbol: "headphones"
                                        badge: "›"
                                        onClicked: root.navigate("audio", this)
                                    }
                                    ControlChoice {
                                        id: outputChoice
                                        flat: true
                                        objectName: "outputChoice"
                                        visible: root.page === "audio"
                                        Layout.fillWidth: true
                                        model: root.outputs
                                        textRole: "description"
                                        currentIndex: root.outputs.findIndex(d => d.name === root.audio.output)
                                        enabled: root.audioFresh && root.outputs.length > 0
                                        Accessible.name: "Audio output"
                                        displayText: currentIndex < 0 ? "No audio output" : currentText
                                        onActivated: index => root.enqueue(["select", "output", root.outputs[index].name])
                                    }
                                    ControlVolumeRow {
                                        id: outputLevel
                                        valueAvailable: !root.rememberedState?.live || root.audioFresh
                                        Layout.fillWidth: true
                                        showMute: true
                                        muteName: "outputMute"
                                        sliderName: "outputVolume"
                                        accessibleLabel: "Output"
                                        muteAccessibleLabel: "Volume"
                                        enabled: root.audioFresh && root.outputs.length > 0
                                        muted: root.output.muted ?? false
                                        volume: root.pendingOutput >= 0 ? root.pendingOutput : root.output.volume ?? 0
                                        onMuteRequested: root.enqueue(["mute", "output", root.output.muted ? "off" : "on"])
                                        onVolumeRequested: value => {
                                            root.pendingOutput = Math.round(value);
                                            root.enqueue(["volume", "output", String(root.pendingOutput)]);
                                        }
                                    }
                                }
                                ControlGroup {
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 1
                                    Text {
                                        visible: root.page === "audio"
                                        text: "Microphone"
                                        color: Material.colors.on_surface
                                        font.family: Theme.typography.family
                                        font.pixelSize: Material.type.titleMedium
                                    }
                                    ControlAction {
                                        Layout.fillWidth: true
                                        visible: root.page === "overview"
                                        objectName: "inputDetails"
                                        text: "Microphone"
                                        detail: root.rememberedState?.live && !root.audioFresh ? "" : root.input.label || root.input.description || "No microphone available"
                                        symbol: "mic"
                                        badge: "›"
                                        onClicked: root.navigate("audio", this)
                                    }
                                    ControlChoice {
                                        id: inputChoice
                                        flat: true
                                        objectName: "inputChoice"
                                        visible: root.page === "audio"
                                        Layout.fillWidth: true
                                        model: root.inputs
                                        textRole: "description"
                                        currentIndex: root.inputs.findIndex(d => d.name === root.audio.input)
                                        displayText: currentIndex < 0 ? "No microphone" : currentText
                                        enabled: root.audioFresh && root.inputs.length > 0
                                        Accessible.name: "Microphone device"
                                        onActivated: index => root.enqueue(["select", "input", root.inputs[index].name])
                                    }
                                    ControlVolumeRow {
                                        id: inputLevel
                                        valueAvailable: !root.rememberedState?.live || root.audioFresh
                                        Layout.fillWidth: true
                                        showMute: true
                                        sliderName: "inputVolume"
                                        accessibleLabel: "Microphone"
                                        muteSymbol: "mic"
                                        mutedSymbol: "mic_off"
                                        enabled: root.audioFresh && root.inputs.length > 0
                                        muted: root.input.muted ?? false
                                        volume: root.pendingInput >= 0 ? root.pendingInput : root.input.volume ?? 0
                                        onMuteRequested: root.enqueue(["mute", "input", root.input.muted ? "off" : "on"])
                                        onVolumeRequested: value => {
                                            root.pendingInput = Math.round(value);
                                            root.enqueue(["volume", "input", String(root.pendingInput)]);
                                        }
                                    }
                                }
                            }
                            ControlGroup {
                                Layout.fillWidth: true
                                visible: root.page === "overview"
                                ControlAction {
                                    objectName: "applicationsToggle"
                                    Layout.fillWidth: true
                                    text: "Application volumes"
                                    badge: "›"
                                    onClicked: {
                                        root.navigate("audio", this);
                                        root.refresh();
                                    }
                                }
                            }
                            ControlGroup {
                                Layout.fillWidth: true
                                visible: root.page === "audio"
                                enabled: root.audioFresh
                                Text {
                                    text: "Application volumes"
                                    color: Material.colors.on_surface
                                    font.family: Theme.typography.family
                                    font.pixelSize: Material.type.titleMedium
                                }
                                Repeater {
                                    model: root.page === "audio" ? applicationModel : null
                                    delegate: ControlVolumeRow {
                                        valueAvailable: !root.rememberedState?.live || root.audioFresh
                                        required property string streamSerial
                                        required property int streamId
                                        required property string streamName
                                        required property string streamLabel
                                        required property real streamVolume
                                        Layout.fillWidth: true
                                        label: streamLabel
                                        accessibleLabel: streamName
                                        sliderName: "applicationVolume-" + streamSerial
                                        volume: root.pendingApplications[streamSerial] ?? streamVolume
                                        onVolumeRequested: value => {
                                            const values = Object.assign({}, root.pendingApplications);
                                            values[streamSerial] = Math.round(value);
                                            root.pendingApplications = values;
                                            root.enqueue(["stream-volume", String(streamId), streamSerial, String(values[streamSerial])]);
                                        }
                                    }
                                }
                            }
                            ControlGroup {
                                Layout.fillWidth: true
                                visible: root.page === "overview"
                                RecordingControl {
                                    Layout.fillWidth: true
                                    recording: root.recording
                                    pending: replayRequest.running
                                    onRequested: requestArgs => root.requestReplay(requestArgs)
                                }
                                ReplayControl {
                                    id: replayControl
                                    showHeading: false
                                    Layout.fillWidth: true
                                    recording: root.recording
                                    pending: replayRequest.running
                                    onSetEnabled: enabled => root.requestReplay(enabled ? "replay-on" : "replay-off")
                                    onSaveRequested: {
                                        root.navigate("replay", replayControl.saveButton);
                                        replayEditor.open();
                                    }
                                }
                            }
                            ReplayEditor {
                                id: replayEditor
                                captureAvailable: root.recording?.state.replay === true && ["buffering", "ready"].includes(root.recording?.state.replay_status) && root.recording?.state.phase !== "error"
                                Layout.fillWidth: true
                                visible: root.page === "replay"
                                backend: root.backend
                                onSettled: root.finishClose()
                            }
                            ControlGroup {
                                Layout.fillWidth: true
                                visible: root.page === "overview"
                                CachyUpdateRow {
                                    id: cachyUpdate
                                    onSettled: root.finishClose()
                                    backend: root.backend
                                    Layout.fillWidth: true
                                }
                            }
                            ControlGroup {
                                Layout.fillWidth: true
                                visible: root.page === "overview"
                                WallpaperGrid {
                                    id: wallpapers
                                    Layout.fillWidth: true
                                    active: !root.closing && root.contentReady && !motion.running && visible
                                    backend: root.backend
                                    viewport: scroll
                                    onStarted: root.error = ""
                                    onFailed: message => root.error = message
                                    onSettled: {
                                        root.finishClose();
                                        root.refresh();
                                    }
                                }
                            }
                            ControlGroup {
                                Layout.fillWidth: true
                                visible: root.page === "network"
                                ControlAction {
                                    Layout.fillWidth: true
                                    objectName: "networkPageToggle"
                                    text: "Networking"
                                    symbol: "lan"
                                    compact: true
                                    toggle: true
                                    badge: root.network.enabled ? "On" : "Off"
                                    marked: root.network.enabled ?? false
                                    enabled: root.networkFresh && (root.network.available ?? false) && !deviceRequest.running
                                    onClicked: root.enqueue(["networking", root.network.enabled ? "off" : "on"])
                                }
                                ControlAction {
                                    Layout.fillWidth: true
                                    objectName: "wifiToggle"
                                    text: "Wi-Fi"
                                    detail: root.wifiAvailable ? "" : "No Wi-Fi adapter"
                                    badge: root.wifiAvailable ? root.network.wifi ? "On" : "Off" : "Unavailable"
                                    marked: root.wifiAvailable && (root.network.wifi ?? false)
                                    enabled: root.networkFresh && root.wifiAvailable && !deviceRequest.running
                                    onClicked: root.enqueue(["wifi", root.network.wifi ? "off" : "on"])
                                }
                                Repeater {
                                    model: root.network.devices ?? []
                                    delegate: ControlAction {
                                        required property var modelData
                                        Layout.fillWidth: true
                                        objectName: "networkDevice-" + modelData.name
                                        text: modelData.kind === "ethernet" ? "Ethernet" : modelData.kind === "wifi" ? "Wi-Fi" : modelData.name
                                        detail: modelData.name + " · " + (modelData.connection && modelData.connection !== "--" ? modelData.connection : modelData.state)
                                        badge: modelData.state === "connected" ? "Disconnect" : "Connect"
                                        marked: modelData.state === "connected"
                                        enabled: root.networkFresh && (root.network.enabled ?? false) && !deviceRequest.running
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
                                        enabled: root.networkFresh && (root.network.enabled ?? false) && !deviceRequest.running
                                        onClicked: root.enqueue(["connection", modelData.active ? "down" : "up", modelData.uuid])
                                    }
                                }
                                ControlAction {
                                    Layout.fillWidth: true
                                    text: "Network settings"
                                    symbol: "settings"
                                    badge: "›"
                                    enabled: root.networkFresh && (root.network.editor ?? false) && !deviceRequest.running
                                    onClicked: {
                                        root.enqueue(["network-settings"]);
                                        root.dismiss();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
