import QtQuick
import QtTest
import Quickshell
import Quickshell.Services.Mpris
import "shell/services"

ShellRoot {
    Component {
        id: controlComponent
        MediaControl {}
    }
    Component {
        id: playerComponent
        QtObject {
            property string dbusName: ""
            property string identity: ""
            property bool canControl: true
            property bool canTogglePlaying: true
            property bool canGoNext: true
            property bool canGoPrevious: true
            property int playbackState: MprisPlaybackState.Paused
            readonly property bool isPlaying: playbackState === MprisPlaybackState.Playing
            property int plays: 0
            property int pauses: 0
            property int nexts: 0
            property int previouses: 0
            function play() {
                plays++;
            }
            function pause() {
                pauses++;
            }
            function next() {
                nexts++;
            }
            function previous() {
                previouses++;
            }
        }
    }
    TestCase {
        name: "MediaSelection"
        when: true
        property var control
        property var browser
        property var spotify
        property var second
        function init() {
            browser = playerComponent.createObject(null, {
                dbusName: "org.mpris.MediaPlayer2.chromium.instance1",
                identity: "Chromium",
                playbackState: MprisPlaybackState.Playing
            });
            spotify = playerComponent.createObject(null, {
                dbusName: "org.mpris.MediaPlayer2.spotify.instance2",
                identity: "Spotify"
            });
            second = playerComponent.createObject(null, {
                dbusName: "org.mpris.MediaPlayer2.vlc",
                identity: "VLC"
            });
            control = controlComponent.createObject(null, {
                players: [browser, spotify, second]
            });
            control.observe(browser);
            control.primaryName = browser.dbusName;
        }
        function cleanup() {
            control.destroy();
            browser.destroy();
            spotify.destroy();
            second.destroy();
        }
        function test_home_prefers_spotify_and_does_not_reassign_primary() {
            compare(control.toggle(true), "Spotify");
            compare(spotify.plays, 1);
            compare(control.toggle(true), "Spotify");
            compare(spotify.pauses, 1);
            spotify.playbackState = MprisPlaybackState.Playing;
            control.observe(spotify);
            compare(control.primaryName, browser.dbusName);
            compare(control.toggle(), "Chromium");
            compare(browser.pauses, 1);
            compare(control.skip(true), "Spotify");
            compare(spotify.nexts, 1);
            compare(control.skip(false), "Spotify");
            compare(spotify.previouses, 1);
            compare(control.primaryName, browser.dbusName);
        }
        function test_navigation_fallback_and_unavailable_tracks() {
            control.players = [browser, second];
            compare(control.skip(true), "Chromium");
            compare(browser.nexts, 1);
            control.players = [browser, spotify];
            spotify.canControl = false;
            compare(control.skip(false), "Chromium");
            compare(browser.previouses, 1);
            spotify.canControl = true;
            spotify.playbackState = MprisPlaybackState.Stopped;
            compare(control.skip(true), "Spotify");
            compare(spotify.nexts, 1);
            spotify.canGoPrevious = false;
            compare(control.skip(false), "Track navigation unavailable");
            compare(browser.previouses, 1);
            control.homePlayer = "vlc";
            compare(control.skip(true), "Spotify");
            control.players = [];
            compare(control.skip(true), "No controllable media player");
        }
        function test_missing_or_uncontrollable_spotify_falls_back() {
            spotify.canControl = false;
            compare(control.toggle(true), "VLC");
            compare(second.plays, 1);
            control.players = [browser, second];
            control.prune();
            compare(control.toggle(true), "VLC");
            compare(second.pauses, 1);
        }
        function test_paused_or_stopped_spotify_session_is_usable() {
            spotify.playbackState = MprisPlaybackState.Stopped;
            compare(control.toggle(true), "Spotify");
            compare(spotify.plays, 1);
            control.players = [spotify];
            compare(control.toggle(true), "Spotify");
            compare(spotify.pauses, 1);
        }
        function test_preference_can_be_changed_or_disabled() {
            control.homePlayer = "vlc";
            compare(control.toggle(true), "VLC");
            control.homePlayer = "";
            control.ranks[second.dbusName] = 100;
            compare(control.toggle(true), "VLC");
            control.players = [];
            compare(control.toggle(true), "No other controllable media player");
        }
        function cleanupTestCase() {
            console.log("MEDIA_CONTROL_TEST_OK");
            Quickshell.quit();
        }
    }
}
