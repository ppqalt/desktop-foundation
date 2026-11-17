import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "shell/components"

ShellRoot {
    Window {
        id: window

        visible: true
        width: 160
        height: 160
        color: "white"

        Repeater {
            id: corners

            model: 4

            ScreenCornerMask {
                required property int index

                corner: index
                cornerRadius: 32
                x: (index === 1 || index === 2) ? 96 : 0
                y: index >= 2 ? 96 : 0
            }

        }

    }

    TestCase {
        function test_corner_geometry_and_pixels() {
            waitForRendering(window.contentItem);
            wait(100);
            for (let index = 0; index < 4; index++) {
                const item = corners.itemAt(index);
                compare(item.width, 32);
                compare(item.height, 32);
                verify(!item.activeFocus);
                const image = grabImage(window.contentItem);
                const right = index === 1 || index === 2;
                const bottom = index >= 2;
                const x = item.x + (right ? 31 : 0);
                const y = item.y + (bottom ? 31 : 0);
                compare(image.pixel(x, y), "#000000", "Outer screen corner must be black");
                const innerX = item.x + (right ? 0 : 31);
                const innerY = item.y + (bottom ? 0 : 31);
                compare(image.pixel(innerX, innerY), "#ffffff", "Inner screen area must remain visible");
                // The center of the cutout square lies inside the circular screen.
                compare(image.pixel(item.x + 16, item.y + 16), "#ffffff");
            }
        }

        function test_panel_transport_smoke() {
            const component = Qt.createComponent(Qt.resolvedUrl("CORNER_SURFACE_FIXTURE"));
            compare(component.status, Component.Ready, component.errorString());
            const scope = component.createObject(null);
            verify(scope !== null);
            wait(50);
            scope.destroy();
        }

        function cleanupTestCase() {
            console.log("SCREEN_CORNERS_TEST_OK");
            Quickshell.quit();
        }

        name: "ScreenCorners"
        when: true
    }

}
