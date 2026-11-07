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
        Timer {
            interval: 1000
            running: true
            onTriggered: {
                if (picker.columnCount !== 4 || picker.entries.length !== 36 || Object.keys(picker.previewUrls).length === 0 || picker.previewUrls[picker.entries[35].path]) {
                    console.error("Initial previews were not bounded to the viewport");
                    Qt.exit(1);
                    return;
                }
                scroll.contentY = scroll.contentHeight - scroll.height;
                finish.start();
            }
        }
        Timer {
            id: finish
            interval: 1000
            onTriggered: {
                if (!picker.previewUrls[picker.entries[35].path]) {
                    console.error("Scrolled-to preview was not loaded");
                    Qt.exit(1);
                    return;
                }
                console.log("LAZY_WALLPAPER_CHECKS_PASSED");
                Qt.quit();
            }
        }
    }
}
