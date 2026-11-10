import QtQuick
import QtTest
import Quickshell
import Quickshell.Io
import "shell/surfaces"
import "shell/theme"

ShellRoot {
    id: fixture
    property int closedCount: 0
    FileView {
        id: calls
        path: Quickshell.env("DF_CONTROL_CALLS")
        blockLoading: true
        printErrors: false
    }
    Component {
        id: panelComponent
        ControlCenterFixture {
            width: 720
            height: 1000
            sidebarStyle: ({
                    width: 720,
                    padding: 12,
                    radius: 14,
                    opacity: 0.75,
                    openingMs: 240,
                    closingMs: 180
                })
            onDismissed: fixture.closedCount++
        }
    }
    TestCase {
        name: "SidebarOpening"
        when: true
        function test_first_frame_gates_motion_and_discovery_waits_for_open() {
            tryCompare(Theme.motionPreference, "running", false, 1500);
            Theme.reducedMotion = false;
            const panel = createTemporaryObject(panelComponent, null);
            verify(panel !== null);
            panel.present();
            // No render-thread discovery before the first frame or during entry.
            compare(panel.snapshot().progress, 0);
            verify(panel.snapshot().moving);
            verify(!panel.contentReady);
            compare(Object.keys(panel.stateData).length, 0);
            calls.reload();
            compare(calls.text(), "");
            wait(50);
            verify(!panel.contentReady);
            compare(Object.keys(panel.stateData).length, 0);
            tryVerify(() => panel.contentReady, 1500);
            compare(panel.snapshot().progress, 1);
            tryVerify(() => !!panel.stateData.audio, 1500);
            panel.dismiss();
            verify(!panel.contentReady);
            tryCompare(fixture, "closedCount", 1, 1500);
            wait(100);
            calls.reload();
            const closedCalls = calls.text();
            wait(2100);
            calls.reload();
            compare(calls.text(), closedCalls);
            const early = createTemporaryObject(panelComponent, null);
            compare(early.snapshot().progress, 0);
            early.present();
            early.dismiss(); // Cancel before mapping without waiting for a frame.
            compare(fixture.closedCount, 2);
            verify(!early.snapshot().moving);
            verify(!early.contentReady);
            wait(50);
            verify(early.closing);
            verify(!early.snapshot().moving);
            console.log("CONTROL_RESULT {\"failures\":0}");
        }
        function cleanupTestCase() {
            Quickshell.quit();
        }
    }
}
