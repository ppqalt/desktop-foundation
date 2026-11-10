import QtQuick
import QtQuick.Window
import Quickshell
import QtTest
import "../../shell/theme"
import "../../shell/components"
import "SURFACE_FIXTURES" as Fixtures

ShellRoot {
    ClockVisibility {
        id: recordingClock
    }
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
            ControlSwitch {
                id: radio
                x: 24
                y: 300
                text: "Bluetooth"
                property int requests: 0
                onClicked: requests++
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
        Fixtures.ModalBackdropFixture {
            id: sharedBackdrop
            width: 800
            height: 800
            property bool popupOne: false
            property bool popupTwo: false
            property int mappings: 0
            requested: popupOne || popupTwo
            onVisibleChanged: {
                if (visible)
                    mappings++;
            }
        }
        Fixtures.VolumeFixture {
            id: volume
        }
        Item {
            id: routeA
            property int dismissals: 0
            property bool alive: false
            property bool superseded: false
            property bool handoff: false
            property bool closing: false
            property bool busy: false
        }
        Item {
            id: routeB
            property bool alive: false
            property bool superseded: false
            property bool handoff: false
            property bool closing: false
            property bool busy: false
        }
        Item {
            id: routeC
            property bool alive: false
            property bool superseded: false
            property bool handoff: false
            property bool closing: false
            property bool busy: false
        }
        SurfaceRouter {
            id: router
            property QtObject sidebar: QtObject {
                property bool alive: false
                property bool closing: false
                property bool superseded: false
            }
            property int externalOpens: 0
            property int externalCloses: 0
            surfaces: ({
                    monitor: {
                        independent: true,
                        external: true,
                        item: () => null,
                        alive: () => false,
                        open: () => router.externalOpens++,
                        dismiss: () => router.externalCloses++
                    },
                    controlCenter: {
                        independent: true,
                        item: () => router.sidebar.alive ? router.sidebar : null,
                        alive: () => router.sidebar.alive,
                        open: () => {
                            router.sidebar.alive = true;
                            router.sidebar.closing = false;
                        },
                        dismiss: () => router.sidebar.closing = true,
                        present: () => router.sidebar.closing = false
                    },
                    first: {
                        item: () => routeA.alive ? routeA : null,
                        alive: () => routeA.alive,
                        open: () => {
                            routeA.superseded = false;
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
                    bluetooth: {
                        item: () => routeC.alive ? routeC : null,
                        alive: () => routeC.alive,
                        open: () => {
                            routeC.alive = true;
                            routeC.superseded = false;
                            routeC.closing = false;
                        },
                        dismiss: () => {
                            routeC.closing = true;
                        },
                        present: () => {
                            routeC.closing = false;
                        }
                    },
                    power: {
                        item: () => routeB.alive ? routeB : null,
                        alive: () => routeB.alive,
                        open: () => {
                            routeB.superseded = false;
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
            property int previews: 0
            property int commits: 0
            property bool mirroredInputs: false
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
                LayoutMirroring.enabled: controls.mirroredInputs
                y: 70
                width: 280
                value: 30
                onPreviewRequested: controls.previews++
                onCommitRequested: controls.commits++
            }
            FoundationRangeSlider {
                id: rangeSlider
                LayoutMirroring.enabled: controls.mirroredInputs
                y: 210
                width: 280
                first.value: 20
                second.value: 80
                onPreviewRequested: controls.previews++
                onCommitRequested: controls.commits++
            }
            ControlSwitch {
                id: testSwitch
                x: 300
                y: 70
                onClicked: controls.activated++
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
            property int disposals: 0
            property int requestResult: -1
            ExternalSurface {
                id: externalWorker
                commandPrefix: ["sh", "-c", "sleep .05; exit 0"]
            }
            function test_external_routing() {
                router.request("first");
                router.request("controlCenter");
                verify(!routeA.superseded && !routeA.closing);
                const opens = router.externalOpens;
                router.request("monitor");
                verify(!routeA.superseded && !routeA.closing);
                verify(router.sidebar.alive && !router.sidebar.closing);
                compare(router.externalOpens, opens + 1);
                const closes = router.externalCloses;
                router.request("bluetooth");
                compare(router.externalCloses, closes);
                verify(routeC.alive && !routeC.superseded);
                verify(!router.sidebar.superseded && !router.sidebar.closing);
                router.request("controlCenter");
                verify(router.sidebar.closing);
                verify(!routeC.closing);
                router.request("controlCenter");
                verify(!router.sidebar.closing);
                router.request("monitor");
                compare(router.externalOpens, opens + 2);
                verify(!router.sidebar.closing && !routeC.closing);
                router.request("power");
                routeB.busy = true;
                router.request("monitor");
                compare(router.externalOpens, opens + 3);
                router.request("controlCenter");
                verify(router.sidebar.closing);
                router.request("controlCenter");
                verify(!router.sidebar.closing);
                routeB.busy = false;
                routeA.alive = false;
                routeB.alive = false;
                routeC.alive = false;
                router.sidebar.alive = false;
                // Close supersedes queued toggles during a slow native startup.
                externalWorker.send("toggle");
                externalWorker.send("toggle");
                externalWorker.send("close");
                compare(externalWorker.queue.join(","), "close");
                externalWorker.send("toggle");
                externalWorker.send("toggle");
                compare(externalWorker.queue.join(","), "close");
                tryCompare(externalWorker, "busy", false);
                compare(externalWorker.worker.command.slice(-1)[0], "close");
            }
            ControlRequest {
                id: transport
                onFinished: code => checks.requestResult = code
            }
            function test_shared_transport() {
                transport.command = ["sh", "-c", "printf invalid"];
                transport.running = true;
                tryCompare(checks, "requestResult", 65);
                compare(transport.phase, "error");
                verify(transport.failure.length > 0);
                requestResult = -1;
                transport.command = ["sh", "-c", "printf '{\"success\":true}'"];
                transport.running = true;
                tryCompare(checks, "requestResult", 0);
                compare(transport.phase, "ready");
                compare(transport.failure, "");
                verify(transport.response.success);
                requestResult = -1;
                transport.expectJson = false;
                transport.command = ["sh", "-c", "exit 0"];
                transport.running = true;
                tryCompare(checks, "requestResult", 0);
            }
            PopupLifecycle {
                id: lifecycle
                onFinished: checks.disposals++
            }
            function test_shared_lifecycle() {
                lifecycle.present();
                lifecycle.framePresented();
                compare(lifecycle.phase, "open");
                lifecycle.closeBlocked = true;
                verify(!lifecycle.dismiss());
                lifecycle.closeBlocked = false;
                lifecycle.canDispose = false;
                verify(lifecycle.dismiss());
                verify(!lifecycle.dismiss());
                wait(Theme.timing.exit + 40);
                compare(disposals, 0);
                lifecycle.present();
                lifecycle.canDispose = true;
                wait(Theme.timing.exit + 40);
                compare(disposals, 0);
                verify(lifecycle.dismiss());
                wait(Theme.timing.exit + 40);
                compare(disposals, 1);
                lifecycle.finishIfReady();
                compare(disposals, 1);
                compare(lifecycle.phase, "disposed");
                compare(Material.listIndex(-1, 4), 0);
                compare(Material.listIndex(4, 4), 3);
            }
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
                compare(router.pending, "");
                verify(routeB.alive);
                verify(routeA.superseded);
                verify(!routeB.superseded);
                const closeCount = routeA.dismissals;
                for (let i = 0; i < 30; i++)
                    router.drain();
                compare(routeA.dismissals, closeCount);
                router.request("first");
                compare(router.pending, "");
                verify(!routeA.closing);
                verify(!routeA.superseded);
                verify(routeB.superseded);
                router.request("power");
                routeA.alive = false;
                router.drain();
                verify(routeB.alive);
                compare(router.pending, "");
                routeB.busy = true;
                router.request("first");
                verify(!routeA.alive);
                verify(!routeB.closing);
                verify(!routeB.superseded);
                routeB.busy = false;
                routeB.alive = false;
                router.request("first");
                router.request("bluetooth");
                verify(routeA.superseded);
                verify(routeC.alive && !routeC.superseded);
                router.request("power");
                verify(routeC.superseded);
                verify(routeB.alive && !routeB.superseded);
                router.request("first");
                verify(!routeA.superseded && !routeA.closing);
                verify(routeB.superseded && routeC.superseded);
                routeA.alive = false;
                routeB.alive = false;
                routeC.alive = false;
            }
            function test_backdrop_survives_handoff() {
                sharedBackdrop.popupOne = true;
                wait(200);
                verify(sharedBackdrop.visible);
                const scrim = find(sharedBackdrop.contentItem, "modal-backdrop-scrim");
                verify(scrim !== null);
                compare(scrim.opacity, 1);
                const mappings = sharedBackdrop.mappings;
                sharedBackdrop.popupOne = false;
                sharedBackdrop.popupTwo = true;
                wait(20);
                verify(sharedBackdrop.visible);
                compare(sharedBackdrop.mappings, mappings);
                compare(scrim.opacity, 1);
                sharedBackdrop.popupTwo = false;
                wait(150);
                verify(!sharedBackdrop.visible);
                card.Window.window.requestActivate();
                wait(20);
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
            function test_control_geometry_tokens() {
                compare(firstControl.implicitHeight, Material.control.listOneLine);
                compare(firstControl.implicitWidth, Material.control.listWidth);
                compare(radio.implicitWidth, Material.control.switchWidth);
                compare(radio.implicitHeight, Material.control.target);
                compare(find(radio, "control-switch-track").width, Material.control.switchTrackWidth);
                compare(find(radio, "control-switch-track").height, Material.control.switchTrackHeight);
                compare(slider.implicitHeight, Material.control.target);
                compare(slider.padding, Material.control.sliderPadding);
                firstControl.detail = "Details";
                compare(firstControl.implicitHeight, Material.control.listTwoLine);
                firstControl.detail = "";
                firstControl.compact = true;
                compare(firstControl.implicitHeight, Material.control.target);
                compare(firstControl.implicitWidth, Material.control.compactButtonWidth);
                firstControl.compact = false;
            }
            function test_large_text_control_geometry() {
                const oldScale = Material.textScale;
                const oldDetail = firstControl.detail;
                const oldCompact = firstControl.compact;
                try {
                    Material.textScale = 2;
                    compare(Material.type.bodyLarge, 32);
                    firstControl.detail = "Supporting text";
                    verify(firstControl.implicitHeight >= Material.type.bodyLargeLine + Material.type.bodyMediumLine + 24);
                    firstControl.detail = "";
                    firstControl.compact = true;
                    verify(firstControl.implicitHeight >= Material.type.labelLine + 16);
                    Material.textScale = 99;
                    compare(Material.type.bodyLarge, 32);
                } finally {
                    Material.textScale = oldScale;
                    firstControl.detail = oldDetail;
                    firstControl.compact = oldCompact;
                }
                compare(firstControl.implicitHeight, Material.control.listOneLine);
            }
            function test_slider_request_semantics() {
                controls.visible = true;
                slider.value = 20;
                controls.previews = 0;
                controls.commits = 0;
                slider.value = 21; // Backend readback is not an input request.
                compare(controls.previews, 0);
                compare(controls.commits, 0);
                slider.forceActiveFocus();
                keyClick(Qt.Key_Right);
                compare(controls.previews, 1);
                compare(controls.commits, 1);
                controls.previews = 0;
                controls.commits = 0;
                mousePress(slider, 100, slider.height / 2);
                mouseMove(slider, 150, slider.height / 2, 20);
                mouseMove(slider, 200, slider.height / 2, 20);
                verify(controls.previews > 0);
                compare(controls.commits, 0);
                mouseRelease(slider, 200, slider.height / 2);
                compare(controls.commits, 1);
                slider.value = 30;
                controls.visible = false;
            }
            function test_range_keyboard_contract() {
                controls.visible = true;
                controls.previews = 0;
                controls.commits = 0;
                rangeSlider.first.value = 20;
                rangeSlider.second.value = 80;
                compare(controls.commits, 0);
                rangeSlider.first.handle.forceActiveFocus();
                keyClick(Qt.Key_D);
                compare(rangeSlider.first.value, 21);
                compare(controls.previews, 1);
                compare(controls.commits, 1);
                keyClick(Qt.Key_Left);
                compare(rangeSlider.first.value, 20);
                rangeSlider.second.handle.forceActiveFocus();
                keyClick(Qt.Key_A);
                compare(rangeSlider.second.value, 79);
                rangeSlider.minimumSpan = 59;
                keyClick(Qt.Key_A);
                compare(rangeSlider.second.value, 79);
                rangeSlider.minimumSpan = 0;
                controls.visible = false;
            }
            function test_slider_rtl_and_disabled_parity() {
                controls.visible = true;
                controls.mirroredInputs = true;
                slider.value = 50;
                rangeSlider.first.value = 20;
                rangeSlider.second.value = 80;
                slider.forceActiveFocus();
                keyClick(Qt.Key_Right);
                compare(slider.value, 49);
                rangeSlider.first.handle.forceActiveFocus();
                keyClick(Qt.Key_D);
                compare(rangeSlider.first.value, 19);
                const commits = controls.commits;
                keyClick(Qt.Key_D, Qt.ControlModifier);
                compare(controls.commits, commits);
                slider.enabled = false;
                rangeSlider.enabled = false;
                mouseClick(slider, 150, slider.height / 2);
                mouseClick(rangeSlider, 150, rangeSlider.height / 2);
                compare(controls.commits, commits);
                slider.enabled = true;
                rangeSlider.enabled = true;
                controls.mirroredInputs = false;
                slider.value = 30;
                controls.visible = false;
            }
            function test_button_switch_activation_parity() {
                controls.visible = true;
                controls.activated = 0;
                for (const control of [firstControl, testSwitch]) {
                    control.forceActiveFocus();
                    const before = controls.activated;
                    keyClick(Qt.Key_Return);
                    keyClick(Qt.Key_Enter);
                    keyPress(Qt.Key_Space);
                    compare(controls.activated, before + 2);
                    keyRelease(Qt.Key_Space);
                    compare(controls.activated, before + 3);
                    const repeated = {
                        key: Qt.Key_Return,
                        modifiers: Qt.NoModifier,
                        isAutoRepeat: true,
                        accepted: false
                    };
                    verify(control.activateKey(repeated));
                    compare(controls.activated, before + 3);
                    control.enabled = false;
                    mouseClick(control, control.width / 2, control.height / 2);
                    compare(controls.activated, before + 3);
                    control.enabled = true;
                }
                controls.activated = 0;
                controls.visible = false;
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
                // Moving out of search must navigate on the very first arrow.
                const nextApplication = launcher.results[1];
                if (nextApplication) {
                    keyClick(Qt.Key_Down);
                    compare(launcher.snapshot().selected, nextApplication.id);
                    keyClick(Qt.Key_Up);
                    keyClick(Qt.Key_Up);
                    verify(launcher.snapshot().inputFocused);
                }
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
                historyMenu.launchSelected();
                wait(200);
                verify(historyMenu.closing);
                verify(!historyMenu.lifecycle.clipboardEnabled);
                historyMenu.lifecycle.clipboardEnabled = true;
                historyMenu.present();
                historyMenu.navigate(1);
                verify(historyMenu.plainAvailable());
                historyMenu.copyPlainSelected();
                wait(200);
                verify(historyMenu.closing);
                verify(!historyMenu.lifecycle.clipboardEnabled);
                historyMenu.visible = false;
            }
            function test_clock_toggle_during_recording_and_finalization() {
                recordingClock.enabled = false;
                recordingClock.recordingPhase = "starting";
                verify(recordingClock.visible);
                recordingClock.recordingPhase = "recording";
                recordingClock.toggle();
                verify(!recordingClock.visible);
                recordingClock.recordingPhase = "stopping";
                verify(!recordingClock.visible);
                recordingClock.toggle();
                verify(recordingClock.visible);
                recordingClock.recordingPhase = "error";
                verify(recordingClock.visible);
                recordingClock.toggle();
                verify(!recordingClock.visible);
                recordingClock.recordingPhase = "idle";
                verify(!recordingClock.visible);
                recordingClock.recordingPhase = "starting";
                verify(recordingClock.visible);
                recordingClock.recordingPhase = "stopping";
                verify(recordingClock.visible);
                recordingClock.recordingPhase = "idle";
                verify(!recordingClock.visible);
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
                volume.presentMessage("Transparency On");
                compare(volume.message, "Transparency On");
                compare(progress.parent.visible, false);
                const noticeIcon = find(volume.contentItem, "audio-notice-icon");
                verify(noticeIcon.source.toString().length > 0);
                compare(noticeIcon.name, "hearing");
                compare(volume.height, 54);
                verify(volume.width < 260);
                volume.presentMessage("Transparency Off");
                compare(noticeIcon.name, "hearing_disabled");
                volume.present(42, false);
                compare(volume.message, "");
                compare(progress.parent.visible, true);
                volume.visible = false;
            }
            function test_shared_roles() {
                compare(card.radius, Material.shape.extraLarge);
                compare(card.color, Material.colors.surface_container_low);
                compare(card.border.width, 1);
                compare(card.border.color, Qt.alpha(Material.colors.primary, 0.65));
                compare(setting.color, Material.colors.secondary_container);
                compare(setting.height, 72);
                compare(clipboard.height, 72);
                compare(search.font.family, "Google Sans Flex");
                compare(search.selectedTextColor, Material.colors.on_secondary_container);
            }
            function test_palette_propagation() {
                const saved = Theme.generated;
                const palette = Object.assign({}, Material.colors, {
                    primary: "#123456"
                });
                try {
                    Theme.generated = Object.assign({}, saved, {
                        material: palette
                    });
                    radio.marked = true;
                    compare(find(radio, "control-switch-track").color, Qt.rgba(18 / 255, 52 / 255, 86 / 255, 1));
                } finally {
                    radio.marked = false;
                    Theme.generated = saved;
                }
            }
            function test_minimal_footer() {
                compare(footer.hintsVisible, false);
                compare(footer.visible, false);
                footer.actionText = "Saving…";
                compare(footer.visible, true);
                footer.actionText = "";
            }
            function test_setting_feedback() {
                const track = find(radio, "control-switch-track");
                const thumb = find(radio, "control-switch-thumb");
                compare(track.width, 52);
                compare(track.height, 32);
                radio.enabled = false;
                compare(track.opacity, Material.state.disabledContent);
                radio.enabled = true;
                compare(thumb.width, 16);
                radio.forceActiveFocus();
                keyClick(Qt.Key_Return);
                compare(radio.requests, 1);
                compare(radio.marked, false);
                radio.marked = true;
                wait(200);
                compare(thumb.width, 24);
                keyClick(Qt.Key_Space);
                compare(radio.requests, 2);
                compare(radio.marked, true);
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
                checks.test_external_routing();
                checks.test_shared_lifecycle();
                checks.test_shared_transport();
                checks.test_backdrop_survives_handoff();
                checks.test_wheel_ownership();
                checks.test_reduced_motion();
                checks.test_shared_roles();
                checks.test_palette_propagation();
                checks.test_control_geometry_tokens();
                checks.test_large_text_control_geometry();
                checks.test_slider_request_semantics();
                checks.test_range_keyboard_contract();
                checks.test_button_switch_activation_parity();
                checks.test_slider_rtl_and_disabled_parity();
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
