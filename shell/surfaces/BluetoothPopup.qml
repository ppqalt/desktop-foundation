pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Bluetooth
import Quickshell.Services.Pipewire
import "../theme"
import "../components"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    required property var lifecycle
    property var targetScreen: null
    property int selected: 0
    property bool closing: false
    property bool entered: false
    Component.onCompleted: Qt.callLater(() => {
        if (!root.closing)
            root.entered = true;
    })
    property bool closeAfterControls: false
    property bool discoveryDone: false
    property string pendingActivation: ""
    property var pendingControls: null
    property var batteryDevice: null
    property var batteryQueue: []
    property var deviceBatteries: ({})
    property var controlDevice: null
    property var controlPaths: []
    property var unsupportedPaths: []
    property bool busy: false
    property var pendingDevice: null
    property string pendingAction: ""
    property string error: ""
    property var result: ({})
    readonly property var devices: Bluetooth.devices.values.filter(d => d.paired || d.bonded).sort((a, b) => a.name.localeCompare(b.name) || a.address.localeCompare(b.address))
    readonly property bool adapterAvailable: Bluetooth.adapters.values.some(a => a.enabled)
    screen: targetScreen
    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-bluetooth"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"
    PwObjectTracker {
        objects: Pipewire.nodes.values
    }
    function deviceIcon(device): string {
        const icon = device.icon || "";
        const name = /headset|headphone/.test(icon) ? "headphones" : /keyboard/.test(icon) ? "keyboard" : /mouse/.test(icon) ? "mouse" : /phone/.test(icon) ? "phone" : /audio/.test(icon) ? "speaker" : /gaming/.test(icon) ? "gamepad" : "";
        return name ? "../assets/bluetooth-" + name + ".svg" : "../assets/bluetooth.svg";
    }
    function audioDetail(device): string {
        const node = Pipewire.nodes.values.find(n => n.isSink && !n.isStream && (String(n.properties["api.bluez5.address"] ?? "").toUpperCase() === device.address.toUpperCase() || n.properties["api.bluez5.path"] === device.dbusPath));
        const codec = node?.properties["api.bluez5.codec"] ?? "";
        const profile = node?.properties["api.bluez5.profile"] ?? "";
        return codec ? String(codec).toUpperCase().replace(/_/g, " ") : profile === "a2dp-sink" ? "A2DP" : profile ? "Headset" : "";
    }
    function detail(device): string {
        if (busy && pendingDevice === device)
            return pendingAction === "disconnect" ? "Disconnecting…" : device.connected ? "Preparing audio…" : "Connecting…";
        const parts = [device.connected ? "Connected" : "Not connected"];
        if (device.connected && audioDetail(device))
            parts.push(audioDetail(device));
        const entry = deviceBatteries[device.dbusPath];
        if (device.connected && entry && Date.now() - entry.at < 120000) {
            const b = entry.battery;
            const values = ["left", "right", "case", "headphone"].filter(k => b[k]).map(k => ({
                        left: "L",
                        right: "R",
                        case: "Case",
                        headphone: "Battery"
                    })[k] + " " + b[k].percent + "%");
            if (values.length)
                parts.push(values.join(" · "));
        } else if (device.batteryAvailable && !controlPaths.includes(device.dbusPath))
            parts.push(Math.round(device.battery * 100) + "%");
        return parts.join(" · ");
    }
    function navigate(delta: int): void {
        selected = Math.max(0, Math.min(devices.length, selected + delta));
        if (selected < devices.length)
            list.positionViewAtIndex(selected, ListView.Contain);
    }
    function dismiss(): void {
        if (controlDevice && controlsLoader.item) {
            closeAfterControls = true;
            controlsLoader.item.leaving = true;
            controlsLoader.item.stop();
            return;
        }
        closing = true;
        closeTimer.start();
    }
    function supportsControls(device): bool {
        return controlPaths.includes(device.dbusPath) && !unsupportedPaths.includes(device.dbusPath);
    }
    function activate(index: int, forceDisconnect = false): void {
        if (busy || closing)
            return;
        selected = index;
        if (index === devices.length) {
            Quickshell.execDetached(["blueman-manager"]);
            dismiss();
            return;
        }
        const device = devices[index];
        if (!device)
            return;
        if (!device.adapter?.enabled || device.blocked) {
            error = device.blocked ? "Device blocked. Open Bluetooth Manager to change this." : "Bluetooth is off. Open Bluetooth Manager to turn it on.";
            return;
        }
        if (device.connected && !discoveryDone && !forceDisconnect) {
            pendingActivation = device.dbusPath;
            return;
        }
        if (device.connected && supportsControls(device) && !forceDisconnect) {
            error = "";
            batteryQueue = [];
            if (batteryProbe.running) {
                pendingControls = device;
                batteryProbe.signal(15);
            } else
                controlDevice = device;
            return;
        }
        pendingDevice = device;
        pendingAction = device.connected ? "disconnect" : "connect";
        busy = true;
        error = "";
        result = {};
        action.command = ["python3", Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/bluetooth-action.py", pendingAction, device.dbusPath];
        action.running = true;
    }
    function snapshot(): var {
        return {
            visible: !closing,
            busy: busy,
            controls: controlsLoader.item ? {
                page: controlsLoader.item.page,
                ready: controlsLoader.item.ready,
                state: controlsLoader.item.earState,
                error: controlsLoader.item.error,
                selected: controlsLoader.item.selected,
                busy: controlsLoader.item.busy,
                scrollY: controlsLoader.item.scrollY,
                hoverNavigationEnabled: controlsLoader.item.hoverNavigationEnabled,
                lastPointer: controlsLoader.item.lastPointer,
                bounds: controlsLoader.item.bounds,
                backendPid: controlsLoader.item.backendPid,
                rows: controlsLoader.item.rows
            } : null,
            selected: selected,
            error: error,
            devices: devices.map(d => ({
                        name: d.name,
                        dbusPath: d.dbusPath,
                        connected: d.connected,
                        battery: d.batteryAvailable ? Math.round(d.battery * 100) : null,
                        audio: audioDetail(d),
                        detail: detail(d)
                    }))
        };
    }
    onDevicesChanged: selected = Math.min(selected, devices.length)
    Process {
        id: discovery
        command: [Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/nothing-backend", "--discover"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.controlPaths = JSON.parse(text).devices || [];
                    root.batteryQueue = root.devices.filter(d => d.connected && root.controlPaths.includes(d.dbusPath));
                    root.nextBattery();
                } catch (_) {}
            }
        }
        onExited: root.finishDiscovery()
    }
    function finishDiscovery(): void {
        discoveryDone = true;
        if (pendingActivation) {
            const index = devices.findIndex(d => d.dbusPath === pendingActivation);
            pendingActivation = "";
            if (index >= 0 && !closing)
                activate(index);
        }
    }
    function forgetBattery(device): void {
        const values = Object.assign({}, deviceBatteries);
        delete values[device.dbusPath];
        deviceBatteries = values;
    }
    function rememberBattery(device, battery): void {
        if (!device)
            return;
        const values = Object.assign({}, deviceBatteries);
        values[device.dbusPath] = {
            battery: battery,
            at: Date.now()
        };
        deviceBatteries = values;
        batteryExpiry.restart();
    }
    function nextBattery(): void {
        if (closing || controlDevice || pendingControls || !batteryQueue.length)
            return;
        batteryDevice = batteryQueue[0];
        batteryQueue = batteryQueue.slice(1);
        batteryProbe.command = [Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/nothing-backend", "--battery", batteryDevice.dbusPath];
        batteryProbe.running = true;
    }
    Process {
        id: batteryProbe
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => {
                try {
                    const message = JSON.parse(data);
                    if (message.event === "battery" && root.batteryDevice?.connected)
                        root.rememberBattery(root.batteryDevice, message.state.battery);
                } catch (_) {}
            }
        }
        onExited: {
            if (root.pendingControls) {
                const device = root.pendingControls;
                root.pendingControls = null;
                if (!root.closing && device.connected)
                    root.controlDevice = device;
            } else
                Qt.callLater(root.nextBattery);
        }
    }
    Timer {
        id: batteryExpiry
        interval: 120000
        running: false
        repeat: false
        onTriggered: root.deviceBatteries = ({})
    }
    Loader {
        id: controlsLoader
        z: 2
        anchors.centerIn: parent
        active: root.controlDevice !== null
        sourceComponent: NothingControls {
            availableWidth: root.width
            availableHeight: root.height
            device: root.controlDevice
            audio: root.audioDetail(root.controlDevice)
            onBatteryReported: battery => root.rememberBattery(root.controlDevice, battery)
            onBack: {
                root.controlDevice = null;
                if (root.closeAfterControls)
                    root.dismiss();
                else
                    card.forceActiveFocus();
            }
            onFailed: message => {
                if (root.controlDevice)
                    root.unsupportedPaths = [...root.unsupportedPaths, root.controlDevice.dbusPath];
                root.error = message.includes("br-connection-create-socket") || message.includes("br-connection-busy") ? "Control channel unavailable. Close other earbud control apps, then reopen Bluetooth." : message;
                root.controlDevice = null;
                card.forceActiveFocus();
            }
            onDisconnect: {
                const index = root.devices.indexOf(root.controlDevice);
                root.controlDevice = null;
                card.forceActiveFocus();
                if (index >= 0)
                    root.activate(index, true);
            }
        }
    }
    Timer {
        id: closeTimer
        interval: Theme.timing.exit
        onTriggered: root.lifecycle.bluetoothEnabled = false
    }
    Process {
        id: action
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.result = JSON.parse(text);
                } catch (_) {
                    root.result = {
                        success: false,
                        error: "Bluetooth request could not be completed."
                    };
                }
            }
        }
        onExited: (exitCode, exitStatus) => {
            root.busy = false;
            if (root.result.success && exitCode === 0) {
                const device = root.pendingDevice;
                const body = root.result.warning || (root.result.connected ? "Connected" : "Disconnected") + (root.result.codec ? " · " + root.result.codec.toUpperCase().replace(/_/g, " ") : "") + (device?.batteryAvailable ? " · " + Math.round(device.battery * 100) + "%" : "");
                Quickshell.execDetached(["notify-send", "--app-name=Bluetooth", "--expire-time=2000", device?.name ?? "Bluetooth", body]);
                if (root.result.connected)
                    root.dismiss();
            } else {
                root.error = root.result.error || "Bluetooth request could not be completed. Try again.";
            }
        }
    }
    Rectangle {
        anchors.fill: parent
        color: Theme.colors.scrim
        opacity: root.entered && !root.closing ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: root.closing ? Theme.timing.exit : Theme.timing.normal
            }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: root.dismiss()
        }
    }
    SurfaceCard {
        id: card
        visible: root.controlDevice === null
        anchors.centerIn: parent
        width: Math.min(Theme.dimensions.launcherWidth, root.width - 48)
        height: Math.min(226 + Math.max(1, Math.min(6, root.devices.length)) * 66 + (root.error ? 38 : 0), root.height - 64)
        opacity: root.entered && !root.closing ? 1 : 0
        scale: root.entered && !root.closing ? 1 : 0.97
        Behavior on opacity {
            NumberAnimation {
                duration: root.closing ? Theme.timing.exit : Theme.timing.normal
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: Theme.timing.normal
                easing.type: Theme.easing
            }
        }
        focus: true
        Component.onCompleted: forceActiveFocus()
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        MouseArea {
            anchors.fill: parent
        }
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Escape)
                root.dismiss();
            else if (event.key === Qt.Key_Up)
                root.navigate(-1);
            else if (event.key === Qt.Key_Down || event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                root.navigate(event.key === Qt.Key_Backtab || event.modifiers & Qt.ShiftModifier ? -1 : 1);
            else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat)
                root.activate(root.selected);
            else
                return;
            event.accepted = true;
        }
        Text {
            x: 28
            y: 23
            text: "BLUETOOTH"
            color: Theme.colors.muted
            font.family: Theme.typography.mono
            font.pixelSize: 10
            font.letterSpacing: 2
        }
        Text {
            anchors.right: parent.right
            anchors.rightMargin: 28
            y: 23
            text: root.devices.length + " paired"
            color: Theme.colors.subtle
            font.family: Theme.typography.mono
            font.pixelSize: 10
        }
        Image {
            x: 28
            y: 60
            width: 24
            height: 28
            source: "../assets/bluetooth.svg"
        }
        Text {
            x: 68
            y: 63
            text: root.adapterAvailable ? "Paired devices" : "Bluetooth is turned off"
            color: Theme.colors.foreground
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.heading
        }
        Rectangle {
            x: 28
            y: 110
            width: card.width - 56
            height: 1
            color: Theme.colors.border
        }
        ListView {
            id: list
            x: 16
            y: 126
            width: card.width - 32
            height: Math.max(1, Math.min(6, root.devices.length)) * 66
            model: root.devices
            clip: true
            interactive: false
            spacing: 4
            SelectionWheel {
                onStepped: delta => root.navigate(delta)
            }
            Text {
                anchors.centerIn: parent
                visible: root.devices.length === 0
                text: "No paired devices yet"
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.body
            }
            delegate: ApplicationRow {
                id: deviceRow
                Connections {
                    target: deviceRow.modelData
                    function onConnectedChanged() {
                        if (!deviceRow.modelData.connected)
                            root.forgetBattery(deviceRow.modelData);
                    }
                }
                required property var modelData
                required property int index
                width: list.width
                entry: ({
                        name: modelData.name || "Paired device",
                        genericName: root.detail(modelData)
                    })
                iconSource: root.deviceIcon(modelData)
                selected: root.selected === index
                actionLabel: root.busy && root.pendingDevice === modelData ? "…" : modelData.connected ? root.supportsControls(modelData) ? "Controls" : "Disconnect" : "Connect"
                onHovered: root.selected = index
                onChosen: root.activate(index)
            }
        }
        Text {
            x: 28
            y: list.y + list.height + 6
            width: card.width - 56
            visible: root.error !== ""
            text: root.error
            color: Theme.colors.error
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
            wrapMode: Text.Wrap
        }
        Rectangle {
            x: 16
            y: card.height - 94
            width: card.width - 32
            height: 36
            radius: Theme.radii.medium
            color: root.selected === root.devices.length ? Theme.colors.selected : manageMouse.containsMouse ? Theme.colors.hover : "transparent"
            border.width: root.selected === root.devices.length ? 1 : 0
            border.color: Theme.colors.selectionBorder
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                text: root.devices.length ? "Manage…" : "Open Bluetooth Manager"
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.body
            }
            Text {
                anchors.right: parent.right
                anchors.rightMargin: 20
                anchors.verticalCenter: parent.verticalCenter
                text: "↗"
                color: Theme.colors.subtle
                font.pixelSize: 18
            }
            MouseArea {
                id: manageMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onEntered: root.selected = root.devices.length
                onClicked: root.activate(root.devices.length)
            }
        }
        Rectangle {
            x: 28
            y: card.height - 48
            width: card.width - 56
            height: 1
            color: Theme.colors.border
        }
        Row {
            x: 28
            y: card.height - 35
            spacing: 7
            Keycap {
                label: "↑ ↓"
            }
            Text {
                text: "navigate"
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 11
                anchors.verticalCenter: parent.verticalCenter
            }
            Item {
                width: 8
                height: 1
            }
            Keycap {
                label: "esc"
            }
            Text {
                text: "close"
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 11
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        Row {
            anchors.right: parent.right
            anchors.rightMargin: 28
            y: card.height - 35
            spacing: 7
            Text {
                text: root.selected === root.devices.length ? "Open manager" : root.devices[root.selected]?.connected ? root.supportsControls(root.devices[root.selected]) ? "Device controls" : "Disconnect device" : "Connect device"
                color: Theme.colors.accent
                font.family: Theme.typography.family
                font.pixelSize: 11
                anchors.verticalCenter: parent.verticalCenter
            }
            Keycap {
                label: "↵"
            }
        }
    }
}
