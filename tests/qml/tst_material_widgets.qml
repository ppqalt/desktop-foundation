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
        Fixtures.LauncherFixture {
            id: launcher
            width: 800
            height: 800
            visible: false
            lifecycle: ({
                    launcherAlive: false,
                    launcherEnabled: true
                })
        }
        Fixtures.ClipboardFixture {
            id: historyMenu
            width: 800
            height: 800
            visible: false
            lifecycle: ({
                    clipboardAlive: false,
                    clipboardEnabled: true
                })
        }
        Fixtures.VolumeFixture {
            id: volume
        }
        Item {
            id: routeA
            property int dismissals: 0
            property bool alive: false
            property bool closing: false
            property bool busy: false
        }
        Item {
            id: routeB
            property bool alive: false
            property bool closing: false
            property bool busy: false
        }
        SurfaceRouter {
            id: router
            surfaces: ({
                    first: {
                        item: () => routeA.alive ? routeA : null,
                        alive: () => routeA.alive,
                        open: () => {
                            routeA.alive = true;
                            routeA.closing = false;
                        },
                        dismiss: () => {
                            routeA.dismissals++;
                            routeA.closing = true;
                        },
                        present: () => {
                            routeA.closing = false;
                        }
                    },
                    power: {
                        item: () => routeB.alive ? routeB : null,
                        alive: () => routeB.alive,
                        open: () => {
                            routeB.alive = true;
                            routeB.closing = false;
                        },
                        dismiss: () => {
                            routeB.closing = true;
                        },
                        present: () => {
                            routeB.closing = false;
                        }
                    }
                })
        }
        Item {
            id: wheelList
            x: 500
            y: 400
            width: 100
            height: 100
            property int steps: 0
            SelectionWheel {
                id: wheel
                onStepped: delta => wheelList.steps += delta
            }
        }
        Item {
            id: sibling
            x: 350
            y: 400
            width: 100
            height: 100
        }
        Item {
            id: controls
            visible: false
            x: 20
            y: 300
            width: 580
            height: 230
            z: 10
            property int activated: 0
            ControlAction {
                id: firstControl
                text: "First"
                width: 240
                onClicked: controls.activated++
            }
            ControlAction {
                id: secondControl
                x: 300
                text: "Second"
                width: 240
                onClicked: controls.activated++
            }
            ControlVolume {
                id: slider
                y: 70
                width: 280
                value: 30
            }
            ControlChoice {
                id: choice
                y: 140
                width: 280
                model: [
                    {
                        name: "One"
                    },
                    {
                        name: "Two"
                    }
                ]
                textRole: "name"
                currentIndex: 0
            }
        }
        SidebarMotion {
            id: sidebar
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
            function test_handoff_and_interruption() {
                router.request("first");
                verify(routeA.alive);
                router.request("first");
                verify(routeA.closing);
                router.request("first");
                verify(!routeA.closing);
                router.request("power");
                compare(router.pending, "power");
                verify(!routeB.alive);
                const closeCount = routeA.dismissals;
                for (let i = 0; i < 30; i++)
                    router.drain();
                compare(routeA.dismissals, closeCount);
                router.request("first");
                compare(router.pending, "");
                verify(!routeA.closing);
                router.request("power");
                routeA.alive = false;
                router.drain();
                verify(routeB.alive);
                compare(router.pending, "");
                routeB.busy = true;
                router.request("first");
                verify(!routeA.alive);
                verify(!routeB.closing);
                routeB.busy = false;
                routeB.alive = false;
            }
            function test_wheel_ownership() {
                mouseWheel(wheelList, 20, 20, 0, -120);
                compare(wheelList.steps, 1);
                mouseWheel(sibling, 20, 20, 0, -120);
                compare(wheelList.steps, 1);
                wheel.accumulated = 60;
                wheelList.visible = false;
                compare(wheel.accumulated, 0);
                wheelList.visible = true;
            }
            function test_reduced_motion() {
                Theme.reducedMotion = true;
                compare(Theme.timing.normal, 0);
                compare(Material.motion.fastSpatial, 0);
                sidebar.move(true);
                compare(sidebar.position, 1);
                verify(!sidebar.running);
                sidebar.move(false);
                compare(sidebar.position, 0);
                verify(!sidebar.running);
                Theme.reducedMotion = false;
            }
            function test_control_navigation() {
                controls.visible = true;
                firstControl.forceActiveFocus();
                keyClick(Qt.Key_D);
                verify(secondControl.activeFocus);
                keyClick(Qt.Key_A);
                verify(firstControl.activeFocus);
                keyClick(Qt.Key_Return);
                compare(controls.activated, 1);
                slider.forceActiveFocus();
                keyClick(Qt.Key_D);
                compare(slider.value, 31);
                keyClick(Qt.Key_Space);
                compare(controls.activated, 1);
                keyClick(Qt.Key_W);
                verify(firstControl.activeFocus);
                choice.forceActiveFocus();
                keyClick(Qt.Key_Space);
                verify(choice.popup.visible);
                keyClick(Qt.Key_S);
                keyClick(Qt.Key_Return);
                compare(choice.currentIndex, 1);
                verify(!choice.popup.visible);
                verify(choice.activeFocus);
                keyClick(Qt.Key_Space);
                keyClick(Qt.Key_Escape);
                verify(!choice.popup.visible);
                verify(choice.activeFocus);
                controls.visible = false;
            }
            function test_search_surface_focus() {
                launcher.visible = true;
                launcher.requestActivate();
                launcher.present();
                wait(20);
                keyClick(Qt.Key_W);
                keyClick(Qt.Key_Space);
                compare(launcher.snapshot().query, "w ");
                launcher.setQuery("");
                keyClick(Qt.Key_Tab);
                verify(!launcher.snapshot().inputFocused);
                keyClick(Qt.Key_Up);
                verify(launcher.snapshot().inputFocused);
                launcher.dismiss();
                launcher.present();
                wait(180);
                verify(!launcher.closing);
                verify(launcher.lifecycle.launcherEnabled);
                launcher.visible = false;
                historyMenu.visible = true;
                historyMenu.requestActivate();
                historyMenu.present();
                wait(20);
                keyClick(Qt.Key_W);
                keyClick(Qt.Key_Space);
                compare(historyMenu.snapshot().query, "w ");
                historyMenu.setQuery("");
                keyClick(Qt.Key_Tab);
                verify(!historyMenu.snapshot().inputFocused);
                keyClick(Qt.Key_S);
                compare(historyMenu.snapshot().query, "");
                historyMenu.beginClear();
                keyClick(Qt.Key_D);
                compare(historyMenu.clearChoice, 1);
                keyClick(Qt.Key_A);
                compare(historyMenu.clearChoice, 0);
                keyClick(Qt.Key_Escape);
                verify(!historyMenu.confirmClear);
                historyMenu.visible = false;
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
                checks.test_handoff_and_interruption();
                checks.test_wheel_ownership();
                checks.test_reduced_motion();
                checks.test_shared_roles();
                checks.test_minimal_footer();
                checks.test_setting_feedback();
                checks.test_search_keyboard();
                checks.test_control_navigation();
                checks.test_search_surface_focus();
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
