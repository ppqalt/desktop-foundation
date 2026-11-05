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
    property string primaryName: ""
    property var pending: ({})

    function requestFor(name) {
        const request = pending[name];
        return request && request.expires > Date.now() ? request : null;
    }

    function observe(player): void {
        const name = player.dbusName;
        const request = requestFor(name);
        if (player.playbackState === MprisPlaybackState.Playing && states[name] !== MprisPlaybackState.Playing) {
            ranks[name] = ++sequence;
            // Home resumes the secondary player without swapping the two keys.
            if (!request?.other)
                primaryName = name;
        }
        states[name] = player.playbackState;
        if (request && player.isPlaying === request.playing)
            delete pending[name];
    }
    function prune(): void {
        const names = new Set(players.map(player => player.dbusName));
        for (const name of Object.keys(states)) {
            if (!names.has(name)) {
                delete states[name];
                delete ranks[name];
                delete pending[name];
            }
        }
        if (primaryName && !names.has(primaryName))
            primaryName = "";
    }
    onPlayersChanged: prune()
    function latest(exclude) {
        let selected = null;
        let rank = -1;
        for (const player of players) {
            if (player.dbusName === exclude || !player.canControl || !player.canTogglePlaying || player.playbackState === MprisPlaybackState.Stopped || player.dbusName === "org.mpris.MediaPlayer2.playerctld")
                continue;
            const candidateRank = ranks[player.dbusName] ?? 0;
            if (candidateRank > rank) {
                selected = player;
                rank = candidateRank;
            }
        }
        return selected;
    }
    function toggle(other = false): string {
        let primary = players.find(player => player.dbusName === primaryName && player.canControl && player.canTogglePlaying && player.playbackState !== MprisPlaybackState.Stopped);
        if (!primary)
            primary = latest("");
        if (!primary)
            return "No controllable media player";
        primaryName = primary.dbusName;
        const selected = other ? latest(primaryName) : primary;
        if (!selected)
            return "No other controllable media player";
        const previous = requestFor(selected.dbusName);
        const playing = !(previous ? previous.playing : selected.isPlaying);
        pending[selected.dbusName] = {
            playing,
            other,
            expires: Date.now() + 1500
        };
        if (playing)
            selected.play();
        else
            selected.pause();
        return selected.identity;
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
