import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import Quickshell.Io
import "shell/components"
import "shell/surfaces"
import "shell/theme"

ShellRoot {
    id: fixture
    property var currentLoader: null
    FileView {
        id: calls
        path: Quickshell.env("DF_CONTROL_CALLS")
        blockLoading: true
        printErrors: false
    }
    Window {
        width: 1
        height: 1
        visible: true
    }
    SurfaceRouter {
        id: router
        surfaces: ({
                controlCenter: {
                    independent: true,
                    item: () => fixture.currentLoader?.surface,
                    alive: () => fixture.currentLoader?.requestedOpen,
                    opening: () => fixture.currentLoader?.requestedOpen,
                    open: () => fixture.currentLoader.present(),
                    dismiss: () => fixture.currentLoader.dismiss(),
                    present: () => fixture.currentLoader.present()
                }
            })
    }
    Component {
        id: loaderComponent
        SidebarLoader {
            id: cached
            ControlCenterFixture {
                startHidden: true
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
                onDismissed: cached.requestedOpen = false
            }
        }
    }
    TestCase {
        name: "SidebarOpening"
        when: true
        function test_prepare_without_mapping_and_reuse_without_polling() {
            const loader = createTemporaryObject(loaderComponent, null);
            fixture.currentLoader = loader;
            verify(loader !== null);
            // This read must not force an unfinished asynchronous load.
            verify(!loader.active);
            compare(loader.surface, null);
            router.request("controlCenter");
            verify(loader.requestedOpen);
            router.request("controlCenter");
            verify(!loader.requestedOpen); // A second cold press cancels intent.
            tryCompare(loader, "active", true, 3000);
            tryCompare(Theme.motionPreference, "running", false, 1500);
            Theme.reducedMotion = true;
            const panel = loader.surface;
            verify(panel !== null);
            verify(!panel.visible);
            verify(panel.closing);
            verify(!panel.snapshot().polling);
            compare(panel.snapshot().progress, 0);
            compare(Object.keys(panel.stateData).length, 0); // No hidden discovery.
            router.request("controlCenter");
            verify(panel.visible);
            verify(!panel.closing);
            compare(panel.snapshot().progress, 1);
            tryVerify(() => !!panel.stateData.audio, 2000);
            router.request("controlCenter");
            verify(!panel.visible);
            verify(panel.closing);
            compare(loader.surface, panel);
            verify(!panel.snapshot().polling);
            wait(100); // Drain only already-issued finite workers.
            calls.reload();
            const closedCalls = calls.text();
            wait(2100); // Hidden sidebar must not restart its two-second refresh.
            calls.reload();
            compare(calls.text(), closedCalls);
            verify(!panel.snapshot().polling);
            for (let i = 0; i < 8; ++i) {
                router.request("controlCenter");
                compare(loader.surface, panel);
                verify(panel.visible);
                router.request("controlCenter");
                verify(!panel.visible);
            }
            console.log("CONTROL_RESULT {\"failures\":0}");
        }
        function cleanupTestCase() {
            Quickshell.quit();
        }
    }
}
