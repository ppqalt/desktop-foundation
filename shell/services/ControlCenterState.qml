import QtQuick
import Quickshell
import Quickshell.Io
import "../components"

// Shell-owned read model; lazy views never own device subscriptions.
// Rust owns event filtering, current readback, reconnection and disk validation.
QtObject {
    id: root
    property bool live: false
    property bool initialized: false
    property var current: ({})
    property int audioEpoch: 0
    property int networkEpoch: 0
    property var snapshot: ({})
    property int revision: 0
    property bool requested: false
    property string pending: ""
    property string inFlight: ""
    property string saved: ""
    property string failed: ""
    property string error: ""
    readonly property bool busy: reader.running || writer.running || pending !== ""
    property string backend: Quickshell.env("DF_FOUNDATION_ROOT") + "/native/foundation/target/release/desktop-foundationctl"
    Component.onCompleted: {
        initialized = true;
        if (live)
            startWatch();
    }
    onLiveChanged: {
        if (!initialized)
            return;
        reconnect.stop();
        if (live)
            startWatch();
        else
            watcher.running = false;
    }
    function startWatch(): void {
        if (!live || watcher.running)
            return;
        watcher.stdinEnabled = true;
        watcher.running = true;
    }
    function send(value): void {
        if (watcher.running)
            watcher.write(JSON.stringify(value) + "\n");
    }
    function epochs(): var {
        return {
            audio_epoch: audioEpoch,
            network_epoch: networkEpoch
        };
    }
    function beginAction(domain): void {
        if (domain === "audio")
            audioEpoch++;
        else
            networkEpoch++;
        send(epochs());
    }
    function publish(value): void {
        current = Object.assign({}, current, value);
        remember(value);
    }
    function confirm(value): void {
        const next = {};
        if (value.audio) {
            audioEpoch++;
            next.audio = value.audio;
        }
        if (value.network) {
            networkEpoch++;
            next.network = value.network;
        }
        send(epochs());
        publish(next);
    }
    function refreshLive(): void {
        send({
            refresh: "all"
        });
        if (!watcher.running)
            startWatch();
    }
    function accept(line): void {
        try {
            const value = JSON.parse(line);
            const next = {};
            if (value.audio && value.audio_epoch === audioEpoch)
                next.audio = value.audio;
            if (value.network && value.network_epoch === networkEpoch)
                next.network = value.network;
            if (Object.keys(next).length)
                publish(next);
        } catch (e) {
            console.warn("Invalid Control Center event:", String(e));
        }
    }
    property Timer reconnect: Timer {
        interval: 1000
        onTriggered: root.startWatch()
    }
    property Process watcher: Process {
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "watch"]
        onStarted: root.send(root.epochs())
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => root.accept(data)
        }
        stderr: StdioCollector {}
        onExited: {
            root.audioEpoch++;
            root.networkEpoch++;
            root.current = {
                audio: {
                    available: false,
                    error: "Event connection unavailable"
                },
                network: {
                    available: false,
                    error: "Event connection unavailable"
                }
            };
            if (root.live)
                root.reconnect.restart();
        }
    }
    function restore(): void {
        if (requested || Object.keys(snapshot).length)
            return;
        requested = true;
        reader.revision = revision;
        reader.running = true;
    }
    function remember(value): void {
        const next = Object.assign({}, snapshot);
        let usable = false;
        for (const key of ["audio", "network"]) {
            if (value[key]?.available === true) {
                next[key] = value[key];
                usable = true;
            }
        }
        if (!usable)
            return;
        revision++;
        snapshot = next;
        pending = JSON.stringify(next);
        if (!flush.running)
            flush.start();
    }
    // Bound burst writes without delaying confirmed UI state or restarting a
    // trailing-edge timer forever during a continuous volume drag.
    property Timer flush: Timer {
        interval: 250
        onTriggered: root.pump()
    }
    function pump(): void {
        if (writer.running || !pending)
            return;
        const payload = pending;
        pending = "";
        if (payload === saved || payload === failed)
            return;
        inFlight = payload;
        writer.stdinEnabled = true;
        writer.running = true;
    }
    property ControlRequest reader: ControlRequest {
        property int revision: 0
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "snapshot-cache", "read"]
        onFinished: code => {
            if (code === 0 && response.snapshot && typeof response.snapshot === "object" && !Array.isArray(response.snapshot)) {
                const next = Object.assign({}, root.snapshot);
                let added = false;
                for (const key of ["audio", "network"]) {
                    if (!next[key] && response.snapshot[key]?.available === true) {
                        next[key] = response.snapshot[key];
                        added = true;
                    }
                }
                if (revision === root.revision) {
                    root.snapshot = next;
                    root.saved = JSON.stringify(next);
                } else if (added) {
                    // A fresh audio result must not discard the last network
                    // observation merely because the cache read finished later.
                    root.snapshot = next;
                    root.pending = JSON.stringify(next);
                    root.pump();
                }
            }
        }
    }
    property Process writer: Process {
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "snapshot-cache", "write"]
        onStarted: {
            write(root.inFlight);
            stdinEnabled = false;
        }
        stdout: StdioCollector {}
        stderr: StdioCollector {
            onStreamFinished: root.error = text.trim().slice(0, 4096)
        }
        onExited: code => {
            if (code === 0) {
                root.saved = root.inFlight;
                root.failed = "";
                root.error = "";
            } else {
                root.failed = root.inFlight;
                console.warn("Could not remember Control Center state:", root.error);
            }
            if (root.pending && !root.flush.running)
                root.flush.start();
        }
    }
}
