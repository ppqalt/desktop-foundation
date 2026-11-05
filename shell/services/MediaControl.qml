pragma ComponentBehavior: Bound
import QtQuick
import QtQml.Models
import Quickshell.Services.Mpris

// Native MPRIS events in the existing shell; no polling or extra media daemon.
QtObject {
    id: root
    readonly property var players: Mpris.players.values
    property var ranks: ({})
    property var states: ({})
    property int sequence: 0
    property string pendingName: ""
    property bool pendingPlaying: false

    function observe(player): void {
        const name = player.dbusName;
        if (player.playbackState === MprisPlaybackState.Playing && states[name] !== MprisPlaybackState.Playing)
            ranks[name] = ++sequence;
        states[name] = player.playbackState;
        if (pendingName === name && player.isPlaying === pendingPlaying) {
            pendingName = "";
            confirmation.stop();
        }
    }
    function prune(): void {
        const names = new Set(players.map(player => player.dbusName));
        for (const name of Object.keys(states)) {
            if (!names.has(name)) {
                delete states[name];
                delete ranks[name];
            }
        }
        if (pendingName && !names.has(pendingName)) {
            pendingName = "";
            confirmation.stop();
        }
    }
    onPlayersChanged: prune()
    function toggle(): string {
        let selected = null;
        let rank = -1;
        for (const player of players) {
            if (!player.canControl || !player.canTogglePlaying || player.playbackState === MprisPlaybackState.Stopped || player.dbusName === "org.mpris.MediaPlayer2.playerctld")
                continue;
            const candidateRank = ranks[player.dbusName] ?? 0;
            if (candidateRank > rank) {
                selected = player;
                rank = candidateRank;
            }
        }
        if (!selected)
            return "No controllable media player";
        const playing = pendingName === selected.dbusName ? pendingPlaying : selected.isPlaying;
        pendingName = selected.dbusName;
        pendingPlaying = !playing;
        if (pendingPlaying)
            selected.play();
        else
            selected.pause();
        confirmation.restart();
        return selected.identity;
    }
    property Timer confirmation: Timer {
        interval: 1500
        onTriggered: root.pendingName = ""
    }
    property Instantiator observers: Instantiator {
        model: root.players
        delegate: Connections {
            required property var modelData
            target: modelData
            Component.onCompleted: root.observe(modelData)
            function onPlaybackStateChanged(): void {
                root.observe(modelData);
            }
        }
    }
}
