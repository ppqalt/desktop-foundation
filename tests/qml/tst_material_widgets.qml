import QtQuick
import QtQuick.Window
import Quickshell
import QtTest
import "../../shell/theme"
import "../../shell/components"
import "SURFACE_FIXTURES" as Fixtures

ShellRoot {
    Window {
        visible: true
        width: 640
        height: 560
        SurfaceCard {
            id: card
            anchors.fill: parent
            anchors.margins: 16
            ApplicationRow {
                id: setting
                x: 16
                y: 80
                width: parent.width - 32
                entry: ({
                        name: "Noise cancellation",
                        genericName: "Adaptive"
                    })
                selected: true
                settingState: true
                onChosen: settingState = !settingState
            }
            ClipboardRow {
                id: clipboard
                x: 16
                y: 160
                width: parent.width - 32
                entry: ({
                        label: "Notes.md",
                        detail: "Markdown",
                        kind: "files",
                        pinned: true
                    })
                selected: false
            }
            SearchInput {
                id: search
                x: 24
                y: 24
                width: parent.width - 48
                height: 48
                placeholderText: "Search"
            }
            SurfaceFooter {
                id: footer
                y: 250
                width: parent.width
            }
        }
        Fixtures.PowerFixture {
            id: session
            width: 800
            height: 800
            visible: false
            lifecycle: ({
                    powerEnabled: true
                })
        }
        Fixtures.VolumeFixture {
            id: volume
        }
        TestCase {
            id: checks
            optional: true
            name: "MaterialWidgets"
            when: false
            function find(item, name) {
                if (item.objectName === name)
                    return item;
                for (const child of item.children || []) {
                    const result = find(child, name);
                    if (result)
                        return result;
                }
                return null;
            }
            function test_session_navigation() {
                session.visible = true;
                session.requestActivate();
                wait(50);
                keyClick(Qt.Key_Down);
                compare(session.selected, 1);
                keyClick(Qt.Key_S);
                compare(session.selected, 2);
                keyClick(Qt.Key_W);
                compare(session.selected, 1);
                keyClick(Qt.Key_Escape);
                compare(session.closing, true);
                session.visible = false;
            }
            function test_volume_feedback() {
                compare(volume.feedbackDuration, 100);
                volume.present(20, false);
                wait(150);
                const progress = find(volume.contentItem, "volume-progress");
                verify(progress !== null);
                volume.present(90, false);
                compare(progress.width, Math.round(progress.parent.width * 0.9));
                volume.present(90, true);
                compare(progress.width, 0);
                volume.visible = false;
            }
            function test_shared_roles() {
                compare(card.radius, Material.shape.extraLarge);
                compare(card.color, Material.colors.surface_container_low);
                compare(setting.color, Material.colors.secondary_container);
                compare(setting.height, 72);
                compare(clipboard.height, 72);
                compare(search.font.family, "Google Sans Flex");
                compare(search.selectedTextColor, Material.colors.on_secondary_container);
            }
            function test_minimal_footer() {
                compare(footer.hintsVisible, false);
                compare(footer.visible, false);
                footer.actionText = "Saving…";
                compare(footer.visible, true);
                footer.actionText = "";
            }
            function test_setting_feedback() {
                compare(setting.settingState, true);
                mouseClick(setting, setting.width / 2, setting.height / 2);
                compare(setting.settingState, false);
                mouseClick(setting, setting.width / 2, setting.height / 2);
                compare(setting.settingState, true);
            }
            function test_search_keyboard() {
                search.forceActiveFocus();
                keyClick(Qt.Key_N);
                keyClick(Qt.Key_O);
                keyClick(Qt.Key_T);
                keyClick(Qt.Key_E);
                compare(search.text, "note");
                keyClick(Qt.Key_A, Qt.ControlModifier);
                compare(search.selectedText, "note");
            }
        }
        Timer {
            interval: 100
            running: true
            onTriggered: {
                checks.test_shared_roles();
                checks.test_minimal_footer();
                checks.test_setting_feedback();
                checks.test_search_keyboard();
                checks.test_session_navigation();
                checks.test_volume_feedback();
                const output = Quickshell.env("DF_MATERIAL_CAPTURE");
                if (output) {
                    card.grabToImage(image => {
                        if (!image.saveToFile(output)) {
                            console.error("Material preview capture failed");
                            Qt.exit(1);
                            return;
                        }
                        console.log("Material widget checks finished");
                        Qt.quit();
                    });
                } else {
                    console.log("Material widget checks finished");
                    Qt.quit();
                }
            }
        }
    }
}
