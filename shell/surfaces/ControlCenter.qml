pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Bluetooth
import "../theme"
import "../components"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    required property var sidebarStyle
    property var targetScreen: null
    property bool closing: false
    property string nextSurface: ""
    property var stateData: ({})
    property string error: ""
    property var queue: []
    property var audioQueue: []
    property bool initialFocus: true
    property int pendingOutput: -1
    property int pendingInput: -1
    readonly property bool busy: deviceRequest.running || audioAction.running
    readonly property var audio: stateData.audio ?? ({})
    readonly property var network: stateData.network ?? ({})
    readonly property bool wifiAvailable: (network.devices ?? []).some(d => d.kind === "wifi")
    readonly property var outputs: audio.outputs ?? []
    readonly property var inputs: audio.inputs ?? []
    readonly property var output: outputs.find(d => d.name === audio.output) ?? ({})
    readonly property var input: inputs.find(d => d.name === audio.input) ?? ({})
    readonly property var paired: Bluetooth.devices.values.filter(d => d.paired || d.bonded).sort((a, b) => a.name.localeCompare(b.name) || a.address.localeCompare(b.address))
    readonly property bool bluetoothAvailable: Bluetooth.adapters.values.length > 0
    readonly property bool bluetoothOn: Bluetooth.adapters.values.some(d => d.enabled)
    readonly property string backend: Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl"
    signal closed(string nextSurface)
    screen: targetScreen
    visible: true
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
    WlrLayershell.keyboardFocus: closing ? WlrKeyboardFocus.None : WlrKeyboardFocus.OnDemand
    color: "transparent"
    Component.onCompleted: Qt.callLater(() => {
        root.present();
        root.refresh();
        outputChoice.forceActiveFocus();
    })
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
            queued: queue.length + audioQueue.length
        };
    }
    function present(): void {
        closing = false;
        nextSurface = "";
        motion.move(true);
    }
    function dismiss(next = ""): void {
        nextSurface = next;
        closing = true;
        status.running = false;
        motion.move(false);
    }
    function finishClose(): void {
        if (closing && !motion.running && !busy && queue.length === 0 && audioQueue.length === 0)
            closed(nextSurface);
    }
    function refresh(): void {
        if (!closing && !busy && queue.length === 0 && audioQueue.length === 0 && !status.running && !outputChoice.popup.visible && !inputChoice.popup.visible)
            status.running = true;
    }
    function enqueue(args: list<string>): void {
        if (closing)
            return;
        const audioRequest = ["volume", "mute", "select"].includes(args[0]);
        let pending = (audioRequest ? audioQueue : queue).slice();
        const existing = args[0] === "volume" ? pending.findIndex(p => p[0] === "volume" && p[1] === args[1]) : -1;
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
        if (code !== 0 || worker.response.success === false)
            error = worker.response.error || worker.failure || "The device request failed. Try again.";
        if (worker.response.audio) {
            stateData = Object.assign({}, stateData, {
                audio: worker.response.audio
            });
            if (!audioQueue.length) {
                pendingOutput = -1;
                pendingInput = -1;
            }
        }
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
    Process {
        id: status
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "snapshot"]
        property var response: ({})
        onStarted: response = ({})
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    status.response = JSON.parse(text);
                } catch (_) {}
            }
        }
        onExited: code => {
            if (root.closing || root.busy || root.queue.length > 0 || root.audioQueue.length > 0)
                return;
            if (code === 0 && response.audio && response.network) {
                root.stateData = response;
                if (root.initialFocus) {
                    root.initialFocus = false;
                    Qt.callLater(() => outputChoice.enabled ? outputChoice.forceActiveFocus() : closeAction.forceActiveFocus());
                }
                if (!outputVolume.pressed)
                    root.pendingOutput = -1;
                if (!inputVolume.pressed)
                    root.pendingInput = -1;
            } else
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
    Rectangle {
        id: card
        anchors.fill: parent
        topLeftRadius: 0
        bottomLeftRadius: 0
        topRightRadius: root.sidebarStyle.radius
        bottomRightRadius: root.sidebarStyle.radius
        property color baseColor: Theme.generated.background ?? "#171b22"
        color: Qt.rgba(baseColor.r, baseColor.g, baseColor.b, root.sidebarStyle.opacity)
        FocusScope {
            id: focusScope
            anchors.fill: parent
            anchors.margins: root.sidebarStyle.padding
            focus: true
            Keys.onEscapePressed: event => {
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
                spacing: 12
                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        Layout.fillWidth: true
                        text: "Control center"
                        color: Theme.colors.foreground
                        font.family: Theme.typography.family
                        font.pixelSize: Theme.typography.heading
                    }
                    Text {
                        text: "Working…"
                        opacity: deviceRequest.running ? 1 : 0
                        color: Theme.colors.muted
                        font.family: Theme.typography.family
                        font.pixelSize: Theme.typography.small
                    }
                    ControlAction {
                        id: closeAction
                        text: "Close"
                        badge: "esc"
                        compact: true
                        implicitWidth: 112
                        onClicked: root.dismiss()
                    }
                }
                Text {
                    Layout.fillWidth: true
                    visible: root.error !== ""
                    text: root.error
                    textFormat: Text.PlainText
                    color: Theme.colors.error
                    wrapMode: Text.Wrap
                    font.family: Theme.typography.family
                    font.pixelSize: Theme.typography.small
                }
                Flickable {
                    id: scroll
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    contentWidth: width
                    contentHeight: body.height
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {
                        visible: size < 1
                        contentItem: Rectangle {
                            implicitWidth: 3
                            radius: 1
                            color: Theme.colors.border
                        }
                        background: null
                    }
                    ColumnLayout {
                        id: body
                        width: scroll.width
                        height: implicitHeight
                        spacing: 8
                        Text {
                            text: "AUDIO"
                            color: Theme.colors.accent
                            font.family: Theme.typography.mono
                            font.pixelSize: Theme.typography.small
                        }
                        Text {
                            Layout.fillWidth: true
                            visible: !root.audio.available
                            text: root.audio.error || "Loading audio…"
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: Theme.colors.muted
                            font.family: Theme.typography.family
                        }
                        ControlChoice {
                            id: outputChoice
                            objectName: "outputChoice"
                            Layout.fillWidth: true
                            model: root.outputs
                            textRole: "description"
                            currentIndex: root.outputs.findIndex(d => d.name === root.audio.output)
                            enabled: root.outputs.length > 0 && !audioAction.running
                            Accessible.name: "Audio output"
                            displayText: currentIndex < 0 ? "No audio output" : currentText
                            onActivated: index => root.enqueue(["select", "output", root.outputs[index].name])
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            ControlAction {
                                text: root.output.muted ? "Output muted" : "Volume"
                                badge: root.output.muted ? "off" : Math.round(outputVolume.value) + "%"
                                marked: outputChoice.currentIndex >= 0 && !root.output.muted
                                enabled: outputChoice.currentIndex >= 0 && !audioAction.running
                                Layout.preferredWidth: 205
                                compact: true
                                onClicked: root.enqueue(["mute", "output", root.output.muted ? "off" : "on"])
                            }
                            ControlVolume {
                                id: outputVolume
                                objectName: "outputVolume"
                                Layout.fillWidth: true
                                enabled: outputChoice.currentIndex >= 0
                                value: root.pendingOutput >= 0 ? root.pendingOutput : root.output.volume ?? 0
                                Accessible.name: "Output volume"
                                onMoved: {
                                    root.pendingOutput = Math.round(value);
                                    root.enqueue(["volume", "output", String(root.pendingOutput)]);
                                }
                            }
                        }
                        ControlChoice {
                            id: inputChoice
                            objectName: "inputChoice"
                            Layout.fillWidth: true
                            model: root.inputs
                            textRole: "description"
                            currentIndex: root.inputs.findIndex(d => d.name === root.audio.input)
                            displayText: currentIndex < 0 ? "No microphone" : currentText
                            enabled: root.inputs.length > 0 && !audioAction.running
                            Accessible.name: "Microphone"
                            onActivated: index => root.enqueue(["select", "input", root.inputs[index].name])
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            ControlAction {
                                text: root.input.muted ? "Microphone muted" : "Microphone"
                                badge: root.input.muted ? "off" : Math.round(inputVolume.value) + "%"
                                marked: inputChoice.currentIndex >= 0 && !root.input.muted
                                enabled: inputChoice.currentIndex >= 0 && !audioAction.running
                                Layout.preferredWidth: 205
                                compact: true
                                onClicked: root.enqueue(["mute", "input", root.input.muted ? "off" : "on"])
                            }
                            ControlVolume {
                                id: inputVolume
                                objectName: "inputVolume"
                                Layout.fillWidth: true
                                enabled: inputChoice.currentIndex >= 0
                                value: root.pendingInput >= 0 ? root.pendingInput : root.input.volume ?? 0
                                Accessible.name: "Microphone volume"
                                onMoved: {
                                    root.pendingInput = Math.round(value);
                                    root.enqueue(["volume", "input", String(root.pendingInput)]);
                                }
                            }
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.topMargin: 8
                            Layout.bottomMargin: 8
                            implicitHeight: 1
                            color: Theme.colors.border
                        }
                        Text {
                            text: "BLUETOOTH"
                            color: Theme.colors.accent
                            font.family: Theme.typography.mono
                            font.pixelSize: Theme.typography.small
                        }
                        ControlAction {
                            Layout.fillWidth: true
                            text: "Bluetooth"
                            badge: root.bluetoothOn ? "on" : "off"
                            marked: root.bluetoothOn
                            detail: root.bluetoothAvailable ? "Paired devices" : "No adapter available"
                            enabled: root.bluetoothAvailable && !deviceRequest.running
                            onClicked: root.enqueue(["bluetooth", "power", root.bluetoothOn ? "off" : "on"])
                        }
                        Repeater {
                            model: root.paired
                            delegate: ControlAction {
                                required property var modelData
                                Layout.fillWidth: true
                                text: modelData.name || modelData.address
                                detail: modelData.connected ? "Connected" : "Paired"
                                badge: modelData.connected ? "disconnect" : "connect"
                                marked: modelData.connected
                                enabled: root.bluetoothOn && !deviceRequest.running
                                onClicked: root.enqueue(["bluetooth", modelData.connected ? "disconnect" : "connect", modelData.dbusPath])
                            }
                        }
                        ControlAction {
                            Layout.fillWidth: true
                            text: "Bluetooth controls"
                            detail: "Device settings, pairing and Nothing controls"
                            badge: "↗"
                            enabled: !deviceRequest.running
                            onClicked: root.dismiss("bluetooth")
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.topMargin: 8
                            Layout.bottomMargin: 8
                            implicitHeight: 1
                            color: Theme.colors.border
                        }
                        Text {
                            text: "NETWORK"
                            color: Theme.colors.accent
                            font.family: Theme.typography.mono
                            font.pixelSize: Theme.typography.small
                        }
                        Text {
                            Layout.fillWidth: true
                            visible: !root.network.available
                            text: root.network.error || "Loading network…"
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: Theme.colors.muted
                            font.family: Theme.typography.family
                        }
                        ControlAction {
                            Layout.fillWidth: true
                            text: "Networking"
                            detail: root.network.state ?? "Unavailable"
                            badge: root.network.enabled ? "on" : "off"
                            marked: root.network.enabled ?? false
                            enabled: (root.network.available ?? false) && !deviceRequest.running
                            onClicked: root.enqueue(["networking", root.network.enabled ? "off" : "on"])
                        }
                        ControlAction {
                            Layout.fillWidth: true
                            text: "Wi-Fi"
                            detail: root.wifiAvailable ? "Wireless radio" : "No Wi-Fi adapter"
                            badge: root.wifiAvailable ? root.network.wifi ? "on" : "off" : "—"
                            marked: root.wifiAvailable && (root.network.wifi ?? false)
                            enabled: root.wifiAvailable && !deviceRequest.running
                            onClicked: root.enqueue(["wifi", root.network.wifi ? "off" : "on"])
                        }
                        Repeater {
                            model: root.network.devices ?? []
                            delegate: ControlAction {
                                required property var modelData
                                Layout.fillWidth: true
                                text: modelData.name
                                detail: modelData.connection && modelData.connection !== "--" ? modelData.connection : modelData.state
                                badge: modelData.state === "connected" ? "disconnect" : "connect"
                                marked: modelData.state === "connected"
                                enabled: (root.network.enabled ?? false) && !deviceRequest.running
                                onClicked: root.enqueue(["device", modelData.state === "connected" ? "disconnect" : "connect", modelData.name])
                            }
                        }
                        ControlAction {
                            Layout.fillWidth: true
                            text: "Network settings"
                            detail: "Add Wi-Fi networks or edit connection credentials"
                            badge: "↗"
                            enabled: (root.network.editor ?? false) && !deviceRequest.running
                            onClicked: {
                                root.enqueue(["network-settings"]);
                                root.dismiss();
                            }
                        }
                        Text {
                            visible: (root.network.connections ?? []).length > 0
                            text: "Saved connections"
                            color: Theme.colors.muted
                            font.family: Theme.typography.family
                            font.pixelSize: Theme.typography.small
                            Layout.topMargin: 8
                        }
                        Repeater {
                            model: root.network.connections ?? []
                            delegate: ControlAction {
                                required property var modelData
                                Layout.fillWidth: true
                                text: modelData.name
                                detail: modelData.kind
                                badge: modelData.active ? "disconnect" : "connect"
                                marked: modelData.active
                                enabled: (root.network.enabled ?? false) && !deviceRequest.running
                                onClicked: root.enqueue(["connection", modelData.active ? "down" : "up", modelData.uuid])
                            }
                        }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    text: "Tab navigate  ·  ← → adjust  ·  Ctrl+R refresh  ·  Esc close"
                    color: Theme.colors.muted
                    font.family: Theme.typography.family
                    font.pixelSize: Theme.typography.small
                }
            }
        }
    }
}
