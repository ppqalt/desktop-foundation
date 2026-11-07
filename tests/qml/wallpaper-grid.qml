import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "../../shell/components"

ShellRoot {
    Window {
        id: window
        visible: true
        width: 720
        height: 600
        color: "#191114"
        property string lastError: ""
        WallpaperGrid {
            id: picker
            x: 24
            y: 24
            width: parent.width - 48
            backend: Quickshell.env("DF_WALLPAPER_BACKEND")
            onFailed: message => window.lastError = message
        }
        TestCase {
            id: keys
            name: "WallpaperGrid"
            optional: true
            when: false
            function find(item, name) {
                if (item.objectName === name)
                    return item;
                for (const child of item.children || []) {
                    const found = find(child, name);
                    if (found)
                        return found;
                }
                return null;
            }
            function runChecks() {
                const first = find(picker, "wallpaper-0");
                const second = find(picker, "wallpaper-1");
                const third = find(picker, "wallpaper-2");
                const fourth = find(picker, "wallpaper-3");
                verify(first && second && third && fourth);
                compare(first.y, third.y);
                verify(fourth.y > third.y);
                compare(first.width, (picker.width - 24) / 3);
                compare(fourth.x, first.x);
                window.requestActivate();
                first.forceActiveFocus();
                wait(20);
                keyClick(Qt.Key_D);
                verify(second.activeFocus);
                keyClick(Qt.Key_D);
                verify(third.activeFocus);
                keyClick(Qt.Key_Right);
                verify(third.activeFocus);
                keyClick(Qt.Key_Left);
                verify(second.activeFocus);
                keyClick(Qt.Key_Space);
                verify(picker.busy);
                picker.active = false;
                verify(picker.busy, "closing must not cancel publication");
                wait(180);
                compare(picker.busy, false);
                compare(picker.currentPath, second.modelData.path);
                picker.active = true;
                wait(40);
                const before = picker.currentPath;
                picker.choose(picker.entries[2]);
                wait(180);
                compare(picker.currentPath, before);
                verify(window.lastError.length > 0);
                // Empty third column stays empty when there are only two files.
                picker.entries = picker.entries.slice(0, 2);
                wait(20);
                compare(find(picker, "wallpaper-0").width, (picker.width - 24) / 3);
            }
        }
        Timer {
            interval: 100
            running: true
            repeat: true
            onTriggered: {
                if (picker.loading || picker.entries.length !== 4)
                    return;
                stop();
                keys.wait(500);
                keys.runChecks();
                picker.grabToImage(result => {
                    // A fully transparent/empty preview must fail this fixture.
                    if (!result.saveToFile(Quickshell.env("DF_WALLPAPER_CAPTURE"))) {
                        console.error("Wallpaper capture failed");
                        Qt.exit(1);
                        return;
                    }
                    console.log("WALLPAPER_CHECKS_PASSED");
                    Qt.quit();
                });
            }
        }
    }
}
