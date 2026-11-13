import QtQuick
import QtQuick.Window
import Quickshell
import "../../shell/components"

ShellRoot {
    Window {
        visible: true
        width: 620
        height: 200
        Flickable {
            id: scroll
            anchors.fill: parent
            contentWidth: width
            contentHeight: picker.implicitHeight
            clip: true
            WallpaperGrid {
                id: picker
                width: scroll.width
                viewport: scroll
                backend: Quickshell.env("DF_WALLPAPER_BACKEND")
            }
        }
        // Observe completion instead of assuming sequential preview workers
        // finish within a fixed second on a loaded CI or virtual machine.
        Timer {
            property bool scrolled: false
            interval: 30
            running: true
            repeat: true
            onTriggered: {
                if (!scrolled) {
                    // This 620x200 viewport intersects two four-tile rows.
                    if (picker.entries.length !== 36 || !picker.previewUrls[picker.entries[7].path])
                        return;
                    if (picker.columnCount !== 4 || picker.previewUrls[picker.entries[35].path]) {
                        console.error("Initial previews were not bounded to the viewport");
                        Qt.exit(1);
                        return;
                    }
                    scroll.contentY = scroll.contentHeight - scroll.height;
                    scrolled = true;
                } else if (picker.previewUrls[picker.entries[35].path]) {
                    console.log("LAZY_WALLPAPER_CHECKS_PASSED");
                    Qt.quit();
                }
            }
        }
        Timer {
            interval: 6000
            running: true
            onTriggered: {
                console.error("Wallpaper preview readiness timed out: entries=" + picker.entries.length + " previews=" + Object.keys(picker.previewUrls).length);
                Qt.exit(1);
            }
        }
    }
}
