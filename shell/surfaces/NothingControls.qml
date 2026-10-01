pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "../theme"
import "../components"

SurfaceCard {
    id: root
    required property var device
    required property real availableWidth
    required property real availableHeight
    required property string audio
    signal back
    signal failed(string message)
    signal disconnect
    property var earState: ({})
    property bool ready: false
    property bool busy: false
    property bool leaving: false
    property string error: ""
    property string page: "main"
    property int selected: 0
    property bool entered: false
    readonly property var ancModes: ["B155", "B171", "B173", "B170"].includes(earState.modelCode) ? [3, 1, 2, 4, 7, 5] : [3, 1, 7, 5]
    readonly property var ancLabels: ({
            1: "ANC · High",
            2: "ANC · Medium",
            3: "ANC · Low",
            4: "ANC · Adaptive",
            5: "Off",
            7: "Transparency"
        })
    readonly property var eqLabels: ({
            0: "Balanced",
            1: "More voice",
            2: "More treble",
            3: "More bass",
            5: "Custom",
            6: "Advanced (device)"
        })
    readonly property var gestureLabels: ({
            1: "No action",
            8: "Previous track",
            9: "Next track",
            10: "Noise control",
            11: "Voice assistant",
            18: "Volume up",
            19: "Volume down",
            20: "ANC / off",
            21: "Transparency / off",
            22: "ANC / transparency"
        })
    readonly property var rows: makeRows()
    width: Math.min(Theme.dimensions.launcherWidth, availableWidth - 48)
    height: Math.min(270 + Math.min(6, rows.length) * 66, availableHeight - 64)
    opacity: entered ? 1 : 0
    scale: entered ? 1 : 0.97
    Behavior on opacity {
        NumberAnimation {
            duration: Theme.timing.normal
        }
    }
    Behavior on scale {
        NumberAnimation {
            duration: Theme.timing.normal
            easing.type: Theme.easing
        }
    }
    focus: true
    Component.onCompleted: {
        forceActiveFocus();
        Qt.callLater(() => root.entered = true);
    }
    function batteryText(): string {
        const b = earState.battery;
        return ["left", "right", "case", "headphone"].filter(k => b?.[k]).map(k => k.charAt(0).toUpperCase() + k.slice(1) + " " + b[k].percent + "%" + (b[k].charging ? " ↑" : "")).join("    ") || "Battery unavailable";
    }
    function row(name, value, key, extra): var {
        return {
            name: name,
            genericName: value,
            key: key,
            extra: extra
        };
    }
    function makeRows(): var {
        const s = earState;
        let result = [];
        if (page === "info")
            return [row("Model", s.modelCode || "Unknown", "none"), row("Firmware", s.firmware || "Unavailable", "none"), row("Bluetooth address", device.address, "none"), row("Advanced equalizer", s.advanced === undefined ? "No reliable response" : "Band controls unavailable in the reference protocol", "none"), row("Back", "Earbud controls", "back")];
        if (page === "find")
            return [row("Remove earbuds from your ears", "Earbuds will emit sound for 3 seconds. Remove them first.", "none"), ...[2, 3].filter(side => s.battery?.[side === 2 ? "left" : "right"]).map(side => row(side === 2 ? "Ring left earbud" : "Ring right earbud", "Emit sound · remove from ears first", "ring", side)), row("Back", "Earbud controls", "back")];
        if (page === "custom")
            return ["Bass", "Mid", "Treble"].map((name, i) => row(name, (s.customEq?.[i] ?? 0) + " dB · ← / → adjust", "customEq", i)).concat([row("Use custom equalizer", "Apply the saved bands", "useCustom"), row("Back", "Earbud controls", "back")]);
        if (page === "gestures")
            return (s.gestures || []).filter(g => [2, 3].includes(g.device) && [2, 3, 7, 9].includes(g.kind)).map(g => row((g.device === 2 ? "Left · " : "Right · ") + ({
                        2: "Double",
                        3: "Triple",
                        7: "Hold",
                        9: "Double and hold"
                    })[g.kind], gestureLabels[g.action] || "Device action " + g.action, "gestures", s.gestures.indexOf(g))).concat([row("Back", "Earbud controls", "back")]);
        if (s.anc != null)
            result.push(row("Noise control", ancLabels[s.anc], "anc"));
        if (s.bass)
            result.push(row("Bass Enhance", (s.bass.enabled ? "On · Level " + s.bass.level : "Off") + " · ← / → level", "bass"));
        if (s.eq != null)
            result.push(row("Equalizer", eqLabels[s.eq] || "Device preset", "eq"));
        if (s.customEq)
            result.push(row("Custom equalizer", "Bass · Mid · Treble", "custom"));
        if (s.listening != null)
            result.push(row("Listening preset", "Device preset " + s.listening + " · ← / → change", "listening"));
        if (s.inEar != null)
            result.push(row("In-ear detection", s.inEar ? "On" : "Off", "inEar"));
        if (s.latency != null)
            result.push(row("Low latency", s.latency ? "On" : "Off", "latency"));
        if (s.gestures?.length)
            result.push(row("Gestures", "Pinch controls", "gesturesPage"));
        if (s.findAvailable && s.modelCode !== "B181")
            result.push(row("Find earbuds", "Emit sound · remove from ears first", "find"));
        result.push(row("Device information", "Model · Firmware", "info"));
        result.push(row("Refresh battery", "Request current values", "refresh"));
        result.push(row("Disconnect", "Return to paired devices", "disconnect"));
        return result;
    }
    onRowsChanged: selected = Math.max(0, Math.min(selected, rows.length - 1))
    function navigate(delta: int): void {
        selected = Math.max(0, Math.min(rows.length - 1, selected + delta));
        list.positionViewAtIndex(selected, ListView.Contain);
    }
    function stop(): void {
        backend.signal(15);
    }
    function goBack(): void {
        if (page !== "main") {
            page = "main";
            selected = 0;
        } else {
            leaving = true;
            backend.signal(15);
        }
    }
    function send(value): void {
        if (!ready || busy || leaving)
            return;
        busy = true;
        error = "";
        backend.write(JSON.stringify(value) + "\n");
        commandDeadline.restart();
    }
    function cycle(values, current, delta): var {
        return values[(Math.max(0, values.indexOf(current)) + delta + values.length) % values.length];
    }
    function activate(index: int, delta: int): void {
        if (!ready || busy || leaving)
            return;
        selected = index;
        const entry = rows[index];
        if (!entry)
            return;
        const key = entry.key;
        if (["info", "custom", "gesturesPage", "find"].includes(key)) {
            page = key === "gesturesPage" ? "gestures" : key;
            selected = 0;
        } else if (key === "back")
            goBack();
        else if (key === "disconnect") {
            leaving = true;
            backend.signal(15);
            root.disconnect();
        } else if (key === "refresh")
            send({
                action: "refresh"
            });
        else if (key === "ring")
            send({
                action: "ring",
                side: entry.extra
            });
        else if (key === "anc")
            send({
                setting: key,
                value: cycle(ancModes, earState.anc, delta || 1)
            });
        else if (key === "eq")
            send({
                setting: key,
                value: cycle(earState.customEq ? [0, 1, 2, 3, 5] : [0, 1, 2, 3], earState.eq, delta || 1)
            });
        else if (key === "listening")
            send({
                setting: key,
                value: cycle([0, 1, 2, 3, 4, 5, 6], earState.listening, delta || 1)
            });
        else if (key === "useCustom")
            send({
                setting: earState.eq !== undefined ? "eq" : "listening",
                value: earState.eq !== undefined ? 5 : 6
            });
        else if (key === "bass")
            send({
                setting: key,
                value: {
                    enabled: delta ? earState.bass.enabled : !earState.bass.enabled,
                    level: delta ? Math.max(1, Math.min(5, earState.bass.level + delta)) : earState.bass.level
                }
            });
        else if (key === "inEar" || key === "latency")
            send({
                setting: key,
                value: !earState[key]
            });
        else if (key === "customEq") {
            const bands = [...earState.customEq];
            bands[entry.extra] = Math.max(-6, Math.min(6, bands[entry.extra] + (delta || 1)));
            send({
                setting: key,
                value: bands
            });
        } else if (key === "gestures") {
            const g = earState.gestures[entry.extra];
            const allowed = g.kind === 7 ? [10, 20, 21, 22, 18, 19, 11] : g.kind === 9 ? [10, 20, 21, 22, 18, 19, 11, 1] : [8, 9, 11];
            send({
                setting: key,
                slot: entry.extra,
                value: cycle(allowed, g.action, delta || 1)
            });
        }
    }
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Escape || event.key === Qt.Key_Back)
            goBack();
        else if (event.key === Qt.Key_Up)
            navigate(-1);
        else if (event.key === Qt.Key_Down || event.key === Qt.Key_Tab)
            navigate(event.modifiers & Qt.ShiftModifier ? -1 : 1);
        else if (event.key === Qt.Key_Left)
            activate(selected, -1);
        else if (event.key === Qt.Key_Right)
            activate(selected, 1);
        else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat)
            activate(selected, 0);
        else
            return;
        event.accepted = true;
    }
    MouseArea {
        anchors.fill: parent
    }
    SelectionWheel {
        onStepped: delta => root.navigate(delta)
    }
    Timer {
        id: commandDeadline
        interval: 6000
        onTriggered: {
            root.busy = false;
            root.error = "Earbuds did not respond. Reopen Controls to reconnect.";
            backend.signal(15);
        }
    }
    Timer {
        id: startupDeadline
        running: !root.ready
        interval: 35000
        onTriggered: {
            root.error = "Earbud controls timed out.";
            backend.signal(15);
        }
    }
    Process {
        id: backend
        command: [Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/nothing-backend", root.device.dbusPath]
        stdinEnabled: true
        running: true
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => {
                try {
                    const message = JSON.parse(data);
                    if (message.state)
                        root.earState = message.state;
                    if (message.event === "ready") {
                        root.ready = true;
                        startupDeadline.stop();
                    }
                    if (message.event === "complete" || message.event === "error") {
                        root.busy = false;
                        commandDeadline.stop();
                    }
                    if (message.event === "error")
                        root.error = message.message;
                    if (message.fatal)
                        root.failed(message.message);
                } catch (_) {
                    root.error = "Invalid earbud response.";
                }
            }
        }
        onExited: {
            if (root.leaving)
                root.back();
            else
                root.failed(root.error || "Earbud controls disconnected. Reconnect and try again.");
        }
    }
    Text {
        x: 28
        y: 23
        text: root.page === "main" ? "EARBUD CONTROLS" : root.page.toUpperCase()
        color: Theme.colors.muted
        font.family: Theme.typography.mono
        font.pixelSize: 10
        font.letterSpacing: 2
    }
    Text {
        x: 28
        y: 60
        width: root.width - 56
        text: root.earState.model || root.device.name
        color: Theme.colors.foreground
        font.family: Theme.typography.family
        font.pixelSize: Theme.typography.heading
    }
    Text {
        x: 28
        y: 93
        text: "Connected" + (root.audio ? " · " + root.audio : "") + (root.ready ? "" : " · Reading controls…")
        color: Theme.colors.muted
        font.family: Theme.typography.family
        font.pixelSize: Theme.typography.small
    }
    Text {
        x: 28
        y: 118
        text: root.batteryText()
        color: Theme.colors.accent
        font.family: Theme.typography.mono
        font.pixelSize: Theme.typography.small
    }
    Rectangle {
        x: 28
        y: 149
        width: root.width - 56
        height: 1
        color: Theme.colors.border
    }
    ListView {
        id: list
        x: 16
        y: 165
        width: root.width - 32
        height: root.height - 257
        clip: true
        interactive: false
        spacing: 4
        model: root.rows
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        delegate: ApplicationRow {
            required property var modelData
            required property int index
            width: list.width
            entry: modelData
            iconSource: "../assets/bluetooth-headphones.svg"
            selected: root.selected === index
            actionLabel: root.busy && root.selected === index ? "…" : ["anc", "eq", "listening", "bass", "customEq", "gestures"].includes(modelData.key) ? "‹  ›" : "↵"
            opacity: root.ready ? 1 : Theme.opacity.disabled
            onHovered: root.selected = index
            onChosen: root.activate(index, 0)
        }
    }
    Text {
        x: 28
        y: root.height - 85
        width: root.width - 56
        text: root.error
        color: Theme.colors.error
        font.family: Theme.typography.family
        font.pixelSize: Theme.typography.small
        wrapMode: Text.Wrap
    }
    Rectangle {
        x: 28
        y: root.height - 48
        width: root.width - 56
        height: 1
        color: Theme.colors.border
    }
    Row {
        x: 28
        y: root.height - 35
        spacing: 7
        Keycap {
            label: "↑ ↓"
        }
        Text {
            text: "navigate"
            color: Theme.colors.muted
            font.family: Theme.typography.family
            font.pixelSize: 11
        }
        Keycap {
            label: "← →"
        }
        Text {
            text: "adjust"
            color: Theme.colors.muted
            font.family: Theme.typography.family
            font.pixelSize: 11
        }
        Keycap {
            label: "esc"
        }
        Text {
            text: "back"
            color: Theme.colors.muted
            font.family: Theme.typography.family
            font.pixelSize: 11
        }
    }
    Text {
        anchors.right: parent.right
        anchors.rightMargin: 28
        y: root.height - 35
        text: "Back ‹"
        color: Theme.colors.accent
        font.family: Theme.typography.family
        font.pixelSize: 11
        MouseArea {
            anchors.fill: parent
            anchors.margins: -8
            cursorShape: Qt.PointingHandCursor
            onClicked: root.goBack()
        }
    }
}
