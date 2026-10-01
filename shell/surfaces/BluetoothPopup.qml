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
        if (device.batteryAvailable)
            parts.push(Math.round(device.battery * 100) + "%");
        return parts.join(" · ");
    }
    function navigate(delta: int): void {
        selected = Math.max(0, Math.min(devices.length, selected + delta));
        if (selected < devices.length)
            list.positionViewAtIndex(selected, ListView.Contain);
    }
    function dismiss(): void {
        closing = true;
        closeTimer.start();
    }
    function activate(index: int): void {
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
            selected: selected,
            error: error,
            devices: devices.map(d => ({
                        name: d.name,
                        connected: d.connected,
                        battery: d.batteryAvailable ? Math.round(d.battery * 100) : null,
                        audio: audioDetail(d)
                    }))
        };
    }
    onDevicesChanged: selected = Math.min(selected, devices.length)
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
            else if (event.key === Qt.Key_Down || event.key === Qt.Key_Tab)
                root.navigate(event.modifiers & Qt.ShiftModifier ? -1 : 1);
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
                required property var modelData
                required property int index
                width: list.width
                entry: ({
                        name: modelData.name || "Paired device",
                        genericName: root.detail(modelData)
                    })
                iconSource: root.deviceIcon(modelData)
                selected: root.selected === index
                actionLabel: root.busy && root.pendingDevice === modelData ? "…" : modelData.connected ? "Disconnect" : "Connect"
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
                text: root.selected === root.devices.length ? "Open manager" : root.devices[root.selected]?.connected ? "Disconnect device" : "Connect device"
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
