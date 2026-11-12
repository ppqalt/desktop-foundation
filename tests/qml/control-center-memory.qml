import QtQuick
import QtTest
import Quickshell
import Quickshell.Io
import "shell/surfaces"
import "shell/services"
import "shell/theme"

ShellRoot {
    id: fixture
    property var memory: null
    property int closed: 0
    FileView {
        id: calls
        path: Quickshell.env("DF_CONTROL_CALLS")
        printErrors: false
        blockLoading: true
    }
    Component {
        id: memoryComponent
        ControlCenterState {}
    }
    Component {
        id: panelComponent
        ControlCenterFixture {
            width: 720
            height: 1000
            rememberedState: fixture.memory
            sidebarStyle: ({
                    width: 720,
                    padding: 12,
                    radius: 14,
                    opacity: 0.75,
                    openingMs: 240,
                    closingMs: 180
                })
            onDismissed: fixture.closed++
        }
    }
    TestCase {
        name: "RememberedControlCenter"
        when: true
        function closePanel(panel, count) {
            panel.dismiss();
            tryCompare(fixture, "closed", count, 1500);
            panel.destroy();
            tryVerify(() => !fixture.memory.busy, 1500);
        }
        function test_last_confirmed_values_survive_views_and_shell_state() {
            tryCompare(Theme.motionPreference, "running", false, 1500);
            Theme.reducedMotion = false;
            fixture.memory = createTemporaryObject(memoryComponent, null);
            const first = createTemporaryObject(panelComponent, null);
            tryVerify(() => first.stateData.audio?.outputs?.[0]?.volume === 50, 1000);
            verify(!first.audioFresh && !first.networkFresh);
            verify(first.refreshingRemembered);
            const slider = findChild(first.contentItem, "outputVolume");
            verify(slider !== null && !slider.enabled);
            first.enqueue(["volume", "output", "4"]);
            first.enqueue(["networking", "off"]);
            compare(first.audioQueue.length, 0);
            compare(first.queue.length, 0);
            tryVerify(() => first.audioFresh && first.networkFresh, 2000);
            compare(first.stateData.audio.outputs[0].volume, 27);
            verify(!first.stateData.network.enabled);
            verify(slider.enabled);
            first.navigate("audio");
            tryVerify(() => !first.snapshot().pageMoving, 1000);
            verify(findChild(first.contentItem, "applicationVolume-200") !== null);
            verify(findChild(first.contentItem, "applicationVolume-100") === null);
            tryVerify(() => !fixture.memory.busy, 1500);
            // Finish accepted actions after the view requests close. The cache
            // owner outlives the panel, without retaining hardware subscriptions.
            first.enqueue(["volume", "output", "73"]);
            closePanel(first, 1);
            compare(fixture.memory.snapshot.audio.outputs[0].volume, 73);
            const second = createTemporaryObject(panelComponent, null);
            tryVerify(() => second.stateData.audio?.outputs?.[0]?.volume === 73, 300);
            verify(!second.audioFresh && !second.networkFresh);
            // The device/service changed while the panel was closed. Fresh data
            // replaces remembered values instead of restoring them to devices.
            tryVerify(() => second.audioFresh && second.networkFresh, 2000);
            compare(second.stateData.audio.outputs[0].volume, 19);
            verify(second.stateData.network.enabled);
            second.enqueue(["networking", "off"]);
            closePanel(second, 2);
            compare(fixture.memory.snapshot.network.enabled, false);
            calls.reload();
            compare(calls.text().split("snapshot-cache read").length - 1, 1);
            verify(!calls.text().includes("volume output 4"));
            compare(calls.text().split("networking off").length - 1, 1);
            fixture.memory.destroy();
            fixture.memory = createTemporaryObject(memoryComponent, null);
            fixture.memory.restore();
            tryVerify(() => !fixture.memory.busy, 1500);
            compare(fixture.memory.snapshot.audio.outputs[0].volume, 19);
            const confirmed = JSON.parse(JSON.stringify(fixture.memory.snapshot));
            fixture.memory.destroy();
            fixture.memory = createTemporaryObject(memoryComponent, null);
            fixture.memory.restore();
            wait(30);
            confirmed.audio.outputs[0].volume = 81;
            fixture.memory.remember(confirmed);
            tryVerify(() => !fixture.memory.busy, 1500);
            compare(fixture.memory.snapshot.audio.outputs[0].volume, 81);
            fixture.memory.destroy();
            fixture.memory = createTemporaryObject(memoryComponent, null);
            fixture.memory.restore();
            wait(30);
            confirmed.audio.outputs[0].volume = 83;
            fixture.memory.remember({
                audio: confirmed.audio
            });
            tryVerify(() => !fixture.memory.busy, 1500);
            compare(fixture.memory.snapshot.audio.outputs[0].volume, 83);
            compare(fixture.memory.snapshot.network.enabled, false);
            const third = createTemporaryObject(panelComponent, null);
            tryVerify(() => third.audioFresh && third.networkFresh, 2000);
            const live = JSON.parse(JSON.stringify(third.stateData));
            third.error = "Action failed";
            verify(third.acceptSnapshot({
                audio: {
                    available: false,
                    error: "Offline"
                },
                network: live.network
            }, third.requestRevision));
            verify(!third.audioFresh && third.networkFresh);
            compare(third.refreshError, "Audio unavailable");
            compare(third.error, "Action failed");
            compare(third.stateData.audio.outputs[0].volume, 19);
            verify(!findChild(third.contentItem, "outputVolume").enabled);
            compare(fixture.memory.snapshot.audio.outputs[0].volume, 19);
            third.enqueue(["volume", "output", "4"]);
            compare(third.audioQueue.length, 0);
            live.audio.outputs = [];
            live.audio.applications = [];
            verify(third.acceptSnapshot(live, third.requestRevision));
            compare(third.refreshError, "");
            compare(third.error, "Action failed");
            compare(third.stateData.audio.outputs.length, 0);
            verify(!findChild(third.contentItem, "outputVolume").enabled);
            verify(!third.acceptSnapshot(confirmed, third.requestRevision - 1));
            compare(fixture.memory.snapshot.audio.outputs.length, 0);
            closePanel(third, 3);
            calls.reload();
            const idle = calls.text();
            wait(300);
            calls.reload();
            compare(calls.text(), idle);
        }
        function cleanupTestCase() {
            console.log('CONTROL_RESULT {"failures":0}');
            Quickshell.quit();
        }
    }
}
