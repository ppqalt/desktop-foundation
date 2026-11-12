import QtQuick
import QtTest
import Quickshell
import Quickshell.Io
import "shell/surfaces"
import "shell/services"
import "shell/theme"

ShellRoot {
    id: fixture
    property int closed: 0
    ControlCenterState {
        id: state
        live: true
    }
    FileView {
        id: calls
        path: Quickshell.env("DF_CONTROL_CALLS")
        printErrors: false
        blockLoading: true
    }
    Component {
        id: panelComponent
        ControlCenterFixture {
            width: 720
            height: 1000
            rememberedState: state
            sidebarStyle: ({
                    width: 720,
                    padding: 12,
                    radius: 14,
                    opacity: .75,
                    openingMs: 240,
                    closingMs: 180
                })
            onDismissed: fixture.closed++
        }
    }
    TestCase {
        name: "LiveControlCenter"
        when: true
        function hasText(item, text) {
            if (typeof item.text === "string" && item.text.includes(text))
                return true;
            return (item.children ?? []).some(child => hasText(child, text));
        }
        function change(volume, enabled) {
            const next = JSON.parse(JSON.stringify(state.current));
            next.audio.outputs[0].volume = volume;
            next.network.enabled = enabled;
            state.send({
                fixture: next
            });
            tryVerify(() => state.current.audio.outputs[0].volume === volume && state.current.network.enabled === enabled, 1000);
        }
        function closePanel(panel, count) {
            panel.dismiss();
            tryCompare(fixture, "closed", count, 1500);
            panel.destroy();
        }
        function test_current_before_first_frame_and_while_closed() {
            const cold = createTemporaryObject(panelComponent, null);
            verify(!cold.audioFresh && !cold.networkFresh);
            verify(!findChild(cold.contentItem, "outputVolume").enabled);
            tryVerify(() => state.snapshot.audio?.outputs?.[0]?.volume === 50, 1000);
            verify(!cold.audioFresh, "disk history must not enable stale values");
            verify(cold.output.volume === undefined);
            verify(!hasText(cold.contentItem, "Refreshing"));
            tryVerify(() => state.current.audio?.available && state.current.network?.available, 1500);
            tryVerify(() => cold.audioFresh && cold.networkFresh, 300);
            compare(cold.output.volume, 27);
            closePanel(cold, 1);
            tryCompare(Theme.motionPreference, "running", false, 1500);
            Theme.reducedMotion = false;
            compare(state.current.audio.outputs[0].volume, 27);
            compare(state.current.network.enabled, false);
            // External device changes arrive before there is a sidebar at all.
            change(63, true);
            const first = createTemporaryObject(panelComponent, null);
            verify(first.audioFresh && first.networkFresh);
            verify(!first.contentReady, "correct values must precede the opening slide's completion");
            compare(first.output.volume, 63);
            compare(first.network.enabled, true);
            compare(first.refreshError, "");
            const slider = findChild(first.contentItem, "outputVolume");
            verify(slider.enabled);
            closePanel(first, 2);
            change(81, false);
            const second = createTemporaryObject(panelComponent, null);
            verify(second.audioFresh && second.networkFresh);
            verify(!second.contentReady);
            compare(second.output.volume, 81);
            compare(second.network.enabled, false);
            // An old native query must never roll back a newer confirmation.
            state.beginAction("audio");
            const old = JSON.parse(JSON.stringify(state.current.audio));
            old.outputs[0].volume = 3;
            state.accept(JSON.stringify({
                audio: old,
                audio_epoch: state.audioEpoch - 1
            }));
            compare(state.current.audio.outputs[0].volume, 81);
            second.enqueue(["volume", "output", "73"]);
            tryVerify(() => !second.busy && !second.audioQueue.length, 1500);
            compare(second.output.volume, 73);
            compare(state.current.audio.outputs[0].volume, 73);
            // A service error does not pretend a saved/zero value is live.
            state.send({
                fixture: {
                    audio: {
                        available: false,
                        error: "Disconnected"
                    }
                }
            });
            tryVerify(() => !second.audioFresh, 1000);
            verify(second.networkFresh);
            compare(second.refreshError, "Audio unavailable");
            verify(!findChild(second.contentItem, "outputVolume").enabled);
            verify(second.output.volume === undefined);
            state.refreshLive();
            tryVerify(() => second.audioFresh, 1000);
            compare(second.output.volume, 73);
            second.navigate("audio");
            tryVerify(() => !second.snapshot().pageMoving && second.page === "audio", 1500);
            const app = findChild(second.contentItem, "applicationVolume-100");
            verify(app !== null);
            mousePress(app, app.width * .5, app.height * .5);
            verify(app.pressed);
            change(73, true);
            verify(app.pressed);
            compare(findChild(second.contentItem, "applicationVolume-100"), app);
            mouseRelease(app, app.width * .6, app.height * .5);
            tryVerify(() => !second.busy, 1000);
            // A failed native session invalidates both domains and reconnects
            // without the view being recreated or accepting epoch-zero output.
            state.send({
                fixture_exit: true
            });
            tryVerify(() => !second.audioFresh && !second.networkFresh, 1000);
            tryVerify(() => second.audioFresh && second.networkFresh, 3000);
            compare(second.output.volume, 73);
            closePanel(second, 3);
            tryVerify(() => !state.busy, 1500);
            calls.reload();
            const idle = calls.text();
            wait(250);
            calls.reload();
            compare(calls.text(), idle, "no closed-view polling");
            verify(!idle.split("\n").includes("snapshot"));
            state.live = false;
        }
        function cleanupTestCase() {
            console.log('CONTROL_RESULT {"failures":0}');
            Quickshell.quit();
        }
    }
}
