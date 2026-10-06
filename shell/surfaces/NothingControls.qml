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
    signal restartRequested
    signal disconnect
    property var earState: ({})
    // Startup and command responses are staged until a complete readback is ready.
    property var incomingState: ({})
    property bool ready: false
    property string pendingRowId: ""
    property var pendingCommand: null
    property bool pendingReadback: false
    property string confirmedRowId: ""
    property var playbackResult: null
    property string playbackReadback: ""
    property bool busy: false
    property bool leaving: false
    property string error: ""
    property string page: "main"
    property int mainSelection: 0
    property bool hoverNavigationEnabled: false
    property point lastPointer: Qt.point(-1, -1)
    readonly property var bounds: ({
            x: (availableWidth - width) / 2,
            y: (availableHeight - height) / 2,
            width: width,
            height: height,
            listY: list.y,
            listHeight: list.height,
            rowHeight: 66
        })
    readonly property var backendPid: backend.processId
    property var playbackCodecs: []
    readonly property string playbackCodec: normalizedCodec(playbackReadback || audio)
    readonly property string playbackLabel: playbackCodec.toUpperCase().replace(/_/g, " ")
    property string requestedPlaybackCodec: "sbc"
    property var previousRows: []
    property string previousPage: ""
    readonly property real scrollY: list.contentY
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
    // Keep the card and footer stable while controls are being discovered.
    height: Math.min(270 + 6 * 66, availableHeight - 64)
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
        if (!ready)
            return "Reading battery…";
        const b = earState.battery;
        return ["left", "right", "case", "headphone"].filter(k => b?.[k]).map(k => k.charAt(0).toUpperCase() + k.slice(1) + " " + b[k].percent + "%" + (b[k].charging ? " ↑" : "")).join("    ") || "Battery unavailable";
    }
    function fitLabel(value): string {
        return value === 0 ? "Good seal" : value === 1 ? "Adjust ear tip" : value === 2 ? "Check earbuds are worn" : "Not tested";
    }
    function row(name, value, key, extra): var {
        const entry = {
            name: name,
            genericName: value,
            key: key,
            extra: extra,
            settingState: null,
            settingValue: ""
        };
        const s = earState;
        if (["inEar", "latency", "dual", "personal", "superMic", "autoTransparency"].includes(key))
            entry.settingState = s[key];
        else if (key === "bass") {
            entry.settingState = s.bass.enabled;
            entry.settingValue = "Level " + s.bass.level;
        } else if (key === "spatial")
            entry.settingState = s.spatial === 1;
        else if (["anc", "eq", "listening", "customEq", "gestures"].includes(key))
            entry.settingValue = key === "anc" ? ancLabels[s.anc] : key === "eq" ? eqLabels[s.eq] || "Device preset" : key === "listening" ? "Preset " + s.listening : key === "customEq" ? (s.customEq?.[extra] ?? 0) + " dB" : gestureLabels[s.gestures[extra].action] || "Device action " + s.gestures[extra].action;
        else if ((key === "quality" && !["sbc", "sbc_xq"].includes(playbackCodec) && s.quality === extra) || (["sbc", "sbc_xq"].includes(key) && playbackCodec === key))
            entry.settingValue = "Selected";
        return entry;
    }
    function makeRows(): var {
        if (!ready)
            return [];
        const s = earState;
        let result = [];
        if (page === "info")
            return [row("Model", s.modelCode || "Unknown", "none"), row("Firmware", s.firmware || "Unavailable", "none"), row("Bluetooth address", device.address, "none"), row("Advanced equalizer", s.advanced === undefined ? "No reliable response" : "Band controls unavailable in the reference protocol", "none"), row("Back", "Earbud controls", "back")];
        if (page === "quality") {
            result = [row("AAC", "Changing quality reboots earbuds", "quality", 0), row("LDAC", "Changing quality reboots earbuds", "quality", 2)];
            for (const codec of ["sbc", "sbc_xq"]) {
                if (playbackCodecs.includes(codec))
                    result.push(row(codec === "sbc" ? "SBC" : "SBC XQ", "Playback on this computer · No earbud reboot", codec));
            }
            return result.concat([row("Current playback", playbackLabel || "Not reported by PipeWire", "none"), row("Back", "Earbud controls", "back")]);
        }
        if (page === "fit")
            return [row("Wear both earbuds", "The test plays sound for about 10 seconds", "none"), row("Start fit test", "Check the seal of each ear tip", "fitStart"), row("Left earbud", fitLabel(s.fit?.left), "none"), row("Right earbud", fitLabel(s.fit?.right), "none"), row("Back", "Earbud controls", "back")];
        if (page === "find")
            return [row("Remove earbuds from your ears", "Earbuds will emit sound for 3 seconds. Remove them first.", "none"), ...[2, 3].filter(side => s.battery?.[side === 2 ? "left" : "right"]).map(side => row(side === 2 ? "Ring left earbud" : "Ring right earbud", "Emit sound · remove from ears first", "ring", side)), row("Back", "Earbud controls", "back")];
        if (page === "custom")
            return s.customEq ? ["Bass", "Mid", "Treble"].map((name, i) => row(name, "← / → adjust", "customEq", i)).concat([row("Use custom equalizer", "Apply the saved bands", "useCustom"), row("Back", "Earbud controls", "back")]) : [row("Equalizer unavailable", "Reopen Controls to read the device again", "none"), row("Back", "Earbud controls", "back")];
        if (page === "gestures")
            return (s.gestures || []).filter(g => [2, 3].includes(g.device) && [2, 3, 7, 9].includes(g.kind)).map(g => row((g.device === 2 ? "Left · " : "Right · ") + ({
                        2: "Double",
                        3: "Triple",
                        7: "Hold",
                        9: "Double and hold"
                    })[g.kind], "← / → change action", "gestures", s.gestures.indexOf(g))).concat([row("Back", "Earbud controls", "back")]);
        if (s.anc != null)
            result.push(row("Noise control", "← / → change mode", "anc"));
        if (s.bass)
            result.push(row("Bass Enhance", "Enter toggle · ← / → level", "bass"));
        if (s.eq != null)
            result.push(row("Equalizer", "← / → change preset", "eq"));
        if (s.customEq)
            result.push(row("Custom equalizer", "Bass · Mid · Treble", "custom"));
        if (s.listening != null)
            result.push(row("Listening preset", "← / → change preset", "listening"));
        if (s.inEar != null)
            result.push(row("In-ear detection", "Pause when an earbud is removed", "inEar"));
        if (s.latency != null)
            result.push(row("Low latency", "Reduce playback delay", "latency"));
        if (s.gestures?.length)
            result.push(row("Gestures", "Pinch controls", "gesturesPage"));
        if (s.findAvailable && s.modelCode !== "B181")
            result.push(row("Find earbuds", "Play a tone to locate your earbuds", "find"));
        for (const entry of [["dual", "Dual connection", "Connect two devices"], ["personal", "Personal sound profile", "Use your existing hearing profile"], ["superMic", "Super Mic", "Use the case microphone"], ["autoTransparency", "Auto-transparency", "Automatic transparency during calls"]]) {
            if (s[entry[0]] != null)
                result.push(row(entry[1], entry[2], entry[0]));
        }
        if (s.quality != null)
            result.push(row("Audio quality", (s.quality === 2 ? "LDAC" : "AAC") + " · Playback: " + (playbackLabel || "unavailable"), "qualityPage"));
        if (s.spatial != null)
            result.push(row("Spatial audio", "Fixed spatial audio", "spatial"));
        if (s.fitAvailable)
            result.push(row("Ear-tip fit test", "Check left and right seal", "fit"));
        result.push(row("Device information", "Model · Firmware", "info"));
        result.push(row("Refresh battery", "Request current values", "refresh"));
        result.push(row("Disconnect", "Return to paired devices", "disconnect"));
        return result;
    }
    onRowsChanged: {
        hoverNavigationEnabled = false;
        const old = previousRows[selected];
        if (previousPage === page && old) {
            const index = rows.findIndex(r => r.key === old.key && r.extra === old.extra);
            selected = index >= 0 ? index : Math.max(0, Math.min(selected, rows.length - 1));
        } else
            selected = Math.max(0, Math.min(selected, rows.length - 1));
        previousRows = rows;
        previousPage = page;
    }
    function navigate(delta: int): void {
        hoverNavigationEnabled = false;
        if (!rows.length)
            return;
        // Informational rows can be focused and scrolled to, but never activated.
        selected = Math.max(0, Math.min(rows.length - 1, selected + delta));
        list.positionViewAtIndex(selected, ListView.Contain);
    }
    function navigateEdge(last: bool): void {
        hoverNavigationEnabled = false;
        if (!rows.length)
            return;
        selected = last ? rows.length - 1 : 0;
        list.positionViewAtIndex(selected, ListView.Contain);
    }
    onEarStateChanged: {
        if (earState.battery)
            batteryReported(earState.battery);
    }
    signal batteryReported(var battery)
    function stop(): void {
        shutdownDeadline.restart();
        backend.signal(15);
        if (codecDiscovery.running)
            codecDiscovery.signal(15);
        if (playbackChange.running)
            playbackChange.signal(15);
    }
    function goBack(): void {
        hoverNavigationEnabled = false;
        if (page !== "main" && ready && !busy) {
            page = "main";
            selected = Math.min(mainSelection, rows.length - 1);
            Qt.callLater(() => list.positionViewAtIndex(root.selected, ListView.Contain));
        } else {
            leaving = true;
            stop();
        }
    }
    function send(value): void {
        if (!ready || busy || leaving)
            return;
        if (value.setting && readbackMatches(value, earState))
            return;
        busy = true;
        const entry = rows[selected];
        pendingRowId = entry.key + ":" + String(entry.extra ?? "");
        pendingCommand = value;
        pendingReadback = false;
        confirmedRowId = "";
        confirmationDeadline.stop();
        error = "";
        backend.write(JSON.stringify(value) + "\n");
        commandDeadline.interval = value.action === "fit" ? 24000 : 6000;
        commandDeadline.restart();
    }
    function cycle(values, current, delta): var {
        return values[(Math.max(0, values.indexOf(current)) + delta + values.length) % values.length];
    }
    function normalizedCodec(value): string {
        return (value || "").trim().toLowerCase().replace(/[-_\s]+/g, "_");
    }
    onAudioChanged: playbackReadback = ""
    function readbackMatches(command, state): bool {
        if (!command?.setting || state[command.setting] == null)
            return false;
        const actual = state[command.setting];
        const wanted = command.value;
        if (command.setting === "bass")
            return actual.enabled === wanted.enabled && actual.level === wanted.level;
        if (command.setting === "customEq")
            return actual.length === wanted.length && actual.every((value, index) => value === wanted[index]);
        if (command.setting === "gestures")
            return actual[command.slot]?.action === wanted;
        return actual === wanted;
    }
    function markConfirmed(rowId: string): void {
        confirmedRowId = rowId;
        confirmationDeadline.restart();
    }
    function handleBackendMessage(message): void {
        if (message.state) {
            incomingState = message.state;
            if (busy)
                pendingReadback = true;
            else if (ready)
                earState = incomingState;
        }
        if (message.event === "ready") {
            earState = incomingState;
            ready = true;
            startupDeadline.stop();
        }
        if (message.event === "complete" || message.event === "error") {
            if (ready)
                earState = incomingState;
            if (message.event === "complete" && message.success === true && pendingReadback && readbackMatches(pendingCommand, incomingState))
                markConfirmed(pendingRowId);
            busy = false;
            pendingCommand = null;
            pendingReadback = false;
            pendingRowId = "";
            commandDeadline.stop();
        }
        if (message.event === "error")
            error = message.message;
        if (message.event === "restart") {
            leaving = true;
            restartRequested();
        }
        if (message.fatal)
            failed(message.message);
    }
    function handlePlaybackResult(result): void {
        playbackResult = result;
        if (!result.success)
            error = result.error || "Playback codec unavailable.";
    }
    function finishPlayback(code: int): void {
        if (ready)
            earState = incomingState;
        if (code === 0 && playbackResult?.success === true && playbackResult.routed === true && normalizedCodec(playbackResult.codec) === requestedPlaybackCodec) {
            // The finite Rust helper verifies the actual sink codec before success.
            playbackReadback = playbackResult.codec;
            markConfirmed(pendingRowId);
        } else if (!error)
            error = "Could not confirm the playback codec.";
        busy = false;
        pendingRowId = "";
        playbackResult = null;
    }
    function pendingLabel(): string {
        return pendingCommand?.action === "fit" ? "Testing…" : pendingCommand?.action === "ring" ? "Ringing…" : pendingCommand?.action === "refresh" ? "Refreshing…" : ["sbc:", "sbc_xq:"].includes(pendingRowId) ? "Switching…" : "Saving…";
    }
    function activate(index: int, delta: int): void {
        if (!ready || busy || leaving)
            return;
        selected = index;
        const entry = rows[index];
        if (!entry)
            return;
        const key = entry.key;
        // Adjustment keys never open a page, restart firmware, ring or disconnect.
        if (delta && !["anc", "eq", "listening", "bass", "spatial", "inEar", "latency", "dual", "personal", "superMic", "autoTransparency", "customEq", "gestures"].includes(key))
            return;
        if (["info", "custom", "gesturesPage", "find", "qualityPage", "fit"].includes(key)) {
            hoverNavigationEnabled = false;
            mainSelection = selected;
            page = key === "gesturesPage" ? "gestures" : key === "qualityPage" ? "quality" : key;
            navigateEdge(false);
            list.positionViewAtBeginning();
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
        else if (key === "fitStart")
            send({
                action: "fit"
            });
        else if (["sbc", "sbc_xq"].includes(key)) {
            busy = true;
            error = "";
            pendingRowId = key + ":";
            requestedPlaybackCodec = key;
            pendingCommand = null;
            playbackResult = null;
            confirmedRowId = "";
            confirmationDeadline.stop();
            playbackChange.running = true;
        } else if (key === "quality")
            send({
                setting: "quality",
                value: entry.extra
            });
        else if (key === "spatial")
            send({
                setting: "spatial",
                value: delta ? (delta > 0 ? 1 : 0) : earState.spatial === 1 ? 0 : 1
            });
        else if (["inEar", "latency", "dual", "personal", "superMic", "autoTransparency"].includes(key))
            send({
                setting: key,
                value: delta ? delta > 0 : !earState[key]
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
    function handleKey(event): void {
        if (event.key === Qt.Key_Escape || event.key === Qt.Key_Back) {
            if (!event.isAutoRepeat)
                goBack();
        } else if (event.key === Qt.Key_Up)
            navigate(-1);
        else if (event.key === Qt.Key_Down || event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
            navigate(event.key === Qt.Key_Backtab || event.modifiers & Qt.ShiftModifier ? -1 : 1);
        else if (event.key === Qt.Key_Left) {
            if (!event.isAutoRepeat)
                activate(selected, -1);
        } else if (event.key === Qt.Key_Right) {
            if (!event.isAutoRepeat)
                activate(selected, 1);
        } else if (event.key === Qt.Key_Home)
            navigateEdge(false);
        else if (event.key === Qt.Key_End)
            navigateEdge(true);
        else if (event.key === Qt.Key_PageUp)
            navigate(-Math.max(1, Math.floor(list.height / 66)));
        else if (event.key === Qt.Key_PageDown)
            navigate(Math.max(1, Math.floor(list.height / 66)));
        else if ([Qt.Key_Return, Qt.Key_Enter, Qt.Key_Space].includes(event.key) && !event.isAutoRepeat)
            activate(selected, 0);
        else
            return;
        event.accepted = true;
    }
    Keys.onPressed: event => handleKey(event)
    MouseArea {
        anchors.fill: parent
    }
    HoverHandler {
        onPointChanged: {
            const p = point.scenePosition;
            if (root.lastPointer.x < 0 || root.lastPointer.y < 0) {
                root.lastPointer = p;
                return;
            }
            if (Math.abs(p.x - root.lastPointer.x) + Math.abs(p.y - root.lastPointer.y) > 1) {
                root.lastPointer = p;
                root.hoverNavigationEnabled = true;
            }
        }
    }
    SelectionWheel {
        onStepped: delta => root.navigate(delta)
    }
    Timer {
        id: confirmationDeadline
        interval: 1400
        onTriggered: root.confirmedRowId = ""
    }
    Timer {
        id: shutdownDeadline
        interval: 1500
        onTriggered: backend.signal(9)
    }
    Timer {
        id: commandDeadline
        interval: 6000
        onTriggered: {
            root.busy = false;
            root.error = "Earbuds did not respond. Reopen Controls to reconnect.";
            backend.signal(15);
            root.failed(root.error);
        }
    }
    Timer {
        id: startupDeadline
        running: !root.ready
        interval: 35000
        onTriggered: {
            root.error = "Earbud controls timed out.";
            backend.signal(15);
            root.failed(root.error);
        }
    }
    Process {
        id: codecDiscovery
        command: [Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl", "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "bluetooth", "codecs", root.device.dbusPath]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const result = JSON.parse(text);
                    if (result.success)
                        root.playbackCodecs = result.codecs || [];
                } catch (_) {}
            }
        }
    }
    Process {
        id: playbackChange
        command: [Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl", "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "bluetooth", "codec", root.device.dbusPath, "--codec", root.requestedPlaybackCodec]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const result = JSON.parse(text);
                    root.handlePlaybackResult(result);
                } catch (_) {
                    root.error = "Could not read the playback codec result.";
                }
            }
        }
        onExited: code => root.finishPlayback(code)
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
                    root.handleBackendMessage(message);
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
        text: "Connected" + (root.playbackLabel ? " · " + root.playbackLabel : "") + (root.ready ? "" : " · Reading controls…")
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
    Item {
        x: 16
        y: 165
        width: root.width - 32
        height: root.height - 257
        clip: true
        visible: !root.ready
        Column {
            width: parent.width
            spacing: 4
            Repeater {
                model: 6
                Rectangle {
                    id: placeholder
                    required property int index
                    width: root.width - 32
                    height: Theme.dimensions.rowHeight
                    radius: Theme.radii.medium
                    color: Theme.colors.elevated
                    opacity: 0.3
                    Rectangle {
                        x: 14
                        anchors.verticalCenter: parent.verticalCenter
                        width: 38
                        height: 38
                        radius: 10
                        color: Theme.colors.border
                    }
                    Rectangle {
                        x: 66
                        y: 19
                        width: 115 + placeholder.index % 3 * 30
                        height: 7
                        radius: 3
                        color: Theme.colors.muted
                    }
                    Rectangle {
                        x: 66
                        y: 35
                        width: 155 + placeholder.index % 2 * 45
                        height: 5
                        radius: 2
                        color: Theme.colors.border
                    }
                }
            }
        }
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
        // A numeric model keeps delegates and scroll position alive across state updates.
        model: root.rows.length
        visible: root.ready
        opacity: root.ready ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.timing.normal
            }
        }
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        delegate: ApplicationRow {
            required property int index
            width: list.width
            entry: root.rows[index] || {
                name: "",
                genericName: "",
                key: "none"
            }
            iconSource: "../assets/bluetooth-headphones.svg"
            selected: root.selected === index
            actionLabel: entry.key === "none" ? "" : ["anc", "eq", "listening", "bass", "customEq", "gestures"].includes(entry.key) ? "‹  ›" : "↵"
            settingState: entry.settingState ?? null
            settingValue: entry.settingValue || ""
            pending: root.busy && root.pendingRowId === entry.key + ":" + String(entry.extra ?? "")
            pendingLabel: root.pendingLabel()
            confirmed: root.confirmedRowId === entry.key + ":" + String(entry.extra ?? "")
            interactive: root.ready && !root.busy && entry.key !== "none"
            onHovered: {
                if (root.hoverNavigationEnabled && index < root.rows.length)
                    root.selected = index;
            }
            onPointerMoved: {
                if (root.hoverNavigationEnabled && index < root.rows.length)
                    root.selected = index;
            }
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
    SurfaceFooter {
        x: 28
        y: root.height - 48
        width: root.width - 56
        adjustable: true
        escapeLabel: "back"
        escapeEnabled: !root.leaving
        onEscapeRequested: root.goBack()
        actionText: !root.ready ? "Reading controls…" : root.busy ? root.pendingLabel() : root.rows[root.selected]?.key === "back" ? "Back" : root.rows[root.selected]?.settingState != null ? "Toggle setting" : "Activate"
        actionEnabled: root.ready && !root.busy && !root.leaving && root.rows[root.selected]?.key !== "none"
        onActivated: root.activate(root.selected, 0)
    }
}
