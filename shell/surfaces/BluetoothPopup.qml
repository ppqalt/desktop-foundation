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
PopupWindow {
    id: root
    required property var lifecycle
    property int selected: -1
    property string selectionKey: ""
    property bool defaultSelectionPending: true
    property bool hoverNavigationEnabled: false
    property point lastPointer: Qt.point(-1, -1)
    Component.onCompleted: Qt.callLater(() => {
        if (!root.closing) {
            root.updateDeviceSelection();
        }
    })
    property bool closeAfterControls: false
    property bool discoveryDone: false
    property string pendingActivation: ""
    property var pendingControls: null
    readonly property bool controlsOpening: pendingActivation !== "" || pendingControls !== null
    property var batteryDevice: null
    property var batteryQueue: []
    property var deviceBatteries: ({})
    property var controlDevice: null
    property var controlPaths: []
    property var unsupportedPaths: []
    property var retryControlsPaths: []
    property string discoveryError: ""
    property var discoveryIssues: []
    property bool discoveryQueued: false
    property bool radioBusy: false
    property bool radioTarget: false
    property bool busy: false
    property var pendingDevice: null
    property string pendingAction: ""
    property string error: ""
    property var result: ({})
    readonly property var devices: Bluetooth.devices.values.filter(d => d.paired || d.bonded || d.trusted || d.connected).sort((a, b) => a.name.localeCompare(b.name) || a.address.localeCompare(b.address))
    readonly property bool adapterAvailable: Bluetooth.adapters.values.some(a => a.enabled)
    surfaceNamespace: "desktop-foundation-bluetooth"
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
        if (pendingControls === device || pendingActivation === device.dbusPath)
            return "Opening controls…";
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
    function toggleRadio(): void {
        if (busy || radioBusy || controlsOpening || closing)
            return;
        const adapters = Bluetooth.adapters.values;
        if (!adapters.length) {
            error = "No Bluetooth adapter is available.";
            return;
        }
        error = "";
        pendingActivation = "";
        radioTarget = !adapterAvailable;
        radioBusy = true;
        radioAction.command = [Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl", "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "bluetooth", "power", radioTarget ? "on" : "off"];
        radioAction.running = true;
    }
    ControlRequest {
        id: radioAction
        onFinished: code => {
            root.radioBusy = false;
            root.error = code === 0 && response.success ? "" : response.error || failure || "Bluetooth radio request failed. Try again.";
        }
    }
    property bool keyboardNavigation: false
    function navigate(delta: int): void {
        // Reading order: radio toggle, devices, then Manage.
        selectIndex(selected + delta);
    }
    function selectIndex(index: int): void {
        keyboardNavigation = true;
        defaultSelectionPending = false;
        hoverNavigationEnabled = false;
        selected = Material.listIndex(index, devices.length + 2, -1);
        if (selected >= 0 && selected < devices.length)
            list.positionViewAtIndex(selected, ListView.Contain);
    }
    function rememberSelection(): void {
        selectionKey = selected === -1 ? "radio" : selected === devices.length ? "manage" : devices[selected]?.dbusPath ?? "";
    }
    function updateDeviceSelection(): void {
        hoverNavigationEnabled = false;
        if (defaultSelectionPending) {
            selected = devices.length ? 0 : -1;
            if (devices.length)
                defaultSelectionPending = false;
        } else {
            const index = devices.findIndex(d => d.dbusPath === selectionKey);
            selected = selectionKey === "radio" ? -1 : selectionKey === "manage" ? devices.length : index >= 0 ? index : devices.length ? Math.min(Math.max(0, selected), devices.length - 1) : -1;
        }
        rememberSelection();
    }
    function pointerSelection(index: int, x: real, y: real): void {
        const moved = lastPointer.x >= 0 && Math.abs(x - lastPointer.x) + Math.abs(y - lastPointer.y) > 1;
        lastPointer = Qt.point(x, y);
        if (moved && !closing && !controlDevice) {
            defaultSelectionPending = false;
            hoverNavigationEnabled = true;
            keyboardNavigation = false;
            selected = index;
        }
    }
    function restoreFocus(): void {
        hoverNavigationEnabled = false;
        lastPointer = Qt.point(-1, -1);
        Qt.callLater(() => {
            if (!root.controlDevice && !root.closing)
                root.restoreInitialFocus();
        });
    }
    function present(): void {
        if (closeAfterControls)
            return;
        beginPresentation();
        restoreFocus();
    }
    function dismiss(): void {
        if (busy || radioBusy || closing)
            return;
        if (controlDevice && controlsLoader.item) {
            closeAfterControls = true;
            controlsLoader.item.leaving = true;
            controlsLoader.item.stop();
            return;
        }
        beginDismissal();
    }
    function supportsControls(device): bool {
        return (controlPaths.includes(device.dbusPath) || retryControlsPaths.includes(device.dbusPath)) && !unsupportedPaths.includes(device.dbusPath);
    }
    function activate(index: int, forceDisconnect = false, capabilitiesChecked = false): void {
        if (busy || radioBusy || controlsOpening || closing)
            return;
        defaultSelectionPending = false;
        selected = index;
        if (index === -1) {
            toggleRadio();
            return;
        }
        if (index === devices.length) {
            Quickshell.execDetached(["blueman-manager"]);
            dismiss();
            return;
        }
        const device = devices[index];
        if (!device)
            return;
        if (!device.adapter?.enabled || device.blocked) {
            error = device.blocked ? "Device blocked" : "Bluetooth is off";
            return;
        }
        if (device.connected && !forceDisconnect && (!discoveryDone || !supportsControls(device) && !capabilitiesChecked)) {
            pendingActivation = device.dbusPath;
            if (discoveryDone)
                refreshDiscovery();
            return;
        }
        if (device.connected && !forceDisconnect && !supportsControls(device) && (discoveryError || discoveryIssues.some(issue => issue.path === device.dbusPath))) {
            error = discoveryError || "Device services could not be read. Select the device to retry.";
            return;
        }
        if (device.connected && supportsControls(device) && !forceDisconnect) {
            error = "";
            hoverNavigationEnabled = false;
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
        action.command = [Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl", "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "bluetooth", pendingAction, device.dbusPath];
        action.running = true;
    }
    function snapshot(): var {
        return {
            visible: !closing,
            busy: busy,
            radioBusy: radioBusy,
            controlsOpening: controlsOpening,
            powered: adapterAvailable,
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
    onSelectedChanged: rememberSelection()
    onDevicesChanged: {
        updateDeviceSelection();
        Qt.callLater(refreshDiscovery);
    }
    function refreshDiscovery(): void {
        if (closing || controlDevice || pendingControls)
            return;
        if (discovery.running) {
            discoveryQueued = true;
            return;
        }
        discoveryDone = false;
        discoveryError = "";
        discovery.running = true;
    }
    ControlRequest {
        id: discovery
        command: [Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/nothing-backend", "--discover"]
        running: true
        onFinished: code => {
            if (code === 0 && !root.closing) {
                root.controlPaths = response.devices || [];
                root.discoveryError = "";
                root.discoveryIssues = response.errors || [];
                root.batteryQueue = root.devices.filter(d => d.connected && root.supportsControls(d));
            } else {
                root.discoveryError = "Could not discover device controls. Select the device to retry.";
                root.batteryQueue = [];
            }
            root.finishDiscovery();
            root.nextBattery();
        }
    }
    function finishDiscovery(): void {
        discoveryDone = true;
        if (discoveryQueued) {
            discoveryQueued = false;
            refreshDiscovery();
            return;
        }
        if (pendingActivation) {
            const index = devices.findIndex(d => d.dbusPath === pendingActivation);
            pendingActivation = "";
            if (index >= 0 && !closing)
                activate(index, false, true);
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
        if (closing || controlDevice || pendingControls || discovery.running || batteryProbe.running || !batteryQueue.length)
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
                    root.restoreFocus();
            }
            onFailed: (message, unsupported) => {
                if (root.controlDevice) {
                    const path = root.controlDevice.dbusPath;
                    if (unsupported)
                        root.unsupportedPaths = [...new Set([...root.unsupportedPaths, path])];
                    else
                        root.retryControlsPaths = [...new Set([...root.retryControlsPaths, path])];
                }
                root.error = message.includes("br-connection-create-socket") || message.includes("br-connection-busy") ? "Control channel unavailable. Close other earbud control apps, then reopen Bluetooth." : message;
                root.controlDevice = null;
                root.restoreFocus();
            }
            onDisconnect: {
                const index = root.devices.indexOf(root.controlDevice);
                root.controlDevice = null;
                root.restoreFocus();
                if (index >= 0)
                    root.activate(index, true);
            }
        }
    }
    motionItem: card
    initialFocusItem: card
    closeBlocked: root.busy || root.radioBusy
    canDispose: !root.busy && !root.radioBusy && !root.controlDevice
    onDisposalReady: root.lifecycle.bluetoothEnabled = false
    onCloseRequested: root.dismiss()
    ControlRequest {
        id: action
        onFinished: exitCode => {
            root.result = response;
            root.busy = false;
            if (root.result.success && exitCode === 0) {
                root.error = "";
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
    PopupCard {
        id: card
        visible: root.controlDevice === null
        anchors.centerIn: parent
        width: Material.popupWidth(Theme.dimensions.deviceMenuWidth, root.width)
        height: Material.popupHeight(128 + Math.max(1, Math.min(6, root.devices.length)) * 76 + (root.error ? 38 : 0) + (root.busy || root.radioBusy || root.controlsOpening ? 48 : 0), root.height)
        presented: root.entered
        dismissing: root.closing
        focus: true
        Keys.priority: Keys.BeforeItem
        MouseArea {
            anchors.fill: parent
        }
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Escape) {
                if (!event.isAutoRepeat)
                    root.dismiss();
            } else if ((event.key === Qt.Key_Up || event.key === Qt.Key_W))
                root.navigate(-1);
            else if (event.key === Qt.Key_Down || event.key === Qt.Key_S || event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                root.navigate(event.key === Qt.Key_Backtab || event.modifiers & Qt.ShiftModifier ? -1 : 1);
            else if (event.key === Qt.Key_Home)
                root.selectIndex(-1);
            else if (event.key === Qt.Key_End)
                root.selectIndex(root.devices.length);
            else if (event.key === Qt.Key_PageUp || event.key === Qt.Key_PageDown)
                root.navigate((event.key === Qt.Key_PageUp ? -1 : 1) * Math.max(1, Math.floor(list.height / 76)));
            else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                if (!event.isAutoRepeat)
                    root.activate(root.selected);
            } else
                return;
            event.accepted = true;
        }
        ControlIcon {
            x: 20
            y: 20
            name: "bluetooth"
            color: Material.colors.primary
        }
        ControlSwitch {
            id: radioControl
            x: 52
            y: 8
            text: "Bluetooth"
            marked: root.adapterAvailable
            pending: root.radioBusy
            selected: root.selected === -1
            keyboardSelection: root.keyboardNavigation
            onPointerFocused: root.keyboardNavigation = false
            activeFocusOnTab: false
            enabled: !root.busy && !root.radioBusy && !root.controlsOpening && Bluetooth.adapters.values.length > 0
            onClicked: root.activate(-1)
            onPointerMoved: (x, y) => root.pointerSelection(-1, x, y)
        }
        ListView {
            id: list
            x: 16
            y: 64
            width: card.width - 32
            height: Math.min(Math.max(1, Math.min(6, root.devices.length)) * 76, Math.max(1, card.height - 128 - (root.error ? 38 : 0) - (root.busy || root.radioBusy || root.controlsOpening ? 48 : 0)))
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
                text: "No saved or connected devices yet"
                color: Material.colors.on_surface_variant
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
                iconName: /headset|headphone/.test(modelData.icon || "") ? "headphones" : ""
                selected: root.selected === index
                actionLabel: (root.busy && root.pendingDevice === modelData) || root.pendingControls === modelData || root.pendingActivation === modelData.dbusPath ? "…" : modelData.connected ? root.supportsControls(modelData) ? "Controls" : "Disconnect" : "Connect"
                onPointerMoved: (x, y) => root.pointerSelection(index, x, y)
                onChosen: root.activate(index)
            }
        }
        WidgetStatus {
            x: 28
            y: list.y + list.height + 6
            width: card.width - 56
            message: root.error
        }
        ControlAction {
            id: manageAction
            x: 16
            y: list.y + list.height + (root.error ? 38 : 0) + 8
            width: card.width - 32
            height: 48
            text: "Settings"
            symbol: "settings"
            marked: root.selected === root.devices.length
            activeFocusOnTab: false
            onClicked: root.activate(root.devices.length)
            HoverHandler {
                onPointChanged: {
                    if (hovered)
                        root.pointerSelection(root.devices.length, point.scenePosition.x, point.scenePosition.y);
                }
            }
        }
        SurfaceFooter {
            x: 28
            y: card.height - 48
            width: card.width - 56
            actionText: root.busy || root.radioBusy || root.controlsOpening ? "Connecting…" : ""
            escapeEnabled: !root.closing
            onEscapeRequested: root.dismiss()
            actionEnabled: !root.closing && !root.busy && !root.radioBusy && !root.controlsOpening
            onActivated: root.activate(root.selected)
        }
    }
}
