import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "shell/components"
import "shell/theme"

ShellRoot {
    Window {
        visible: true
        width: 540
        height: 1300
        ControlAction {
            id: action
            objectName: "animated-action"
            x: 200
            width: 160
            text: "Open"
            compact: true
        }
        ControlSwitch {
            id: toggle
            text: "Motion switch"
            onClicked: marked = !marked
        }
        FoundationSlider {
            id: slider
            y: 60
            width: 400
            value: 50
        }
        ControlChoice {
            id: choice
            x: 410
            width: 120
            flat: true
            model: ["Desktop", "Microphone"]
        }
        MotionGallery {
            id: gallery
            y: 115
            width: 500
        }
        WallpaperGrid {
            id: wallpapers
            y: 900
            width: 500
            backend: Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/component-gallery-fixture"
        }
        StateLayer {
            id: pulseExample
            width: 48
            height: 48
            anchors.fill: undefined
            color: "white"
        }
        Item {
            id: presentation
        }
        Rectangle {
            id: effects
            property bool lit: false
            color: lit ? "white" : "black"
            opacity: lit ? 1 : 0
            Behavior on color {
                SurfaceColorAnimation {}
            }
            Behavior on opacity {
                SurfaceAnimation {}
            }
        }
    }
    Component {
        id: springComponent
        SpringValue {
            property int completions: 0
            onSettled: completions++
        }
    }
    SidebarMotion {
        id: sidebar
        property int completions: 0
        onSettled: completions++
    }
    PopupLifecycle {
        id: popup
        targetItem: presentation
        property int completions: 0
        onFinished: completions++
    }
    ContentTransition {
        id: pages
        property int commits: 0
        property int completions: 0
        property bool committedWhileVisible: false
        onCommitted: {
            commits++;
            committedWhileVisible = committedWhileVisible || opacity !== 0;
        }
        onSettled: completions++
    }
    TestCase {
        name: "SharedMotion"
        when: true
        function initTestCase() {
            tryCompare(Theme.motionPreference, "running", false, 1500);
            Theme.reducedMotion = false;
        }
        function init() {
            Theme.reducedMotion = false;
        }
        function test_button_pointer_keyboard_and_repeated_feedback() {
            const feedback = action.transform[0];
            mousePress(action, action.width / 2, action.height / 2);
            wait(40);
            verify(feedback.xScale < 1 && feedback.yScale < 1);
            mouseRelease(action, action.width / 2, action.height / 2);
            tryCompare(feedback, "compression", 0, 1500);
            action.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_Return);
            verify(feedback.pulsing); // Return emits clicked without native down.
            wait(30);
            keyClick(Qt.Key_Return);
            tryCompare(feedback, "compression", 0, 1500);
            keyClick(Qt.Key_Space);
            Theme.reducedMotion = true;
            compare(feedback.compression, 0);
            verify(!feedback.pulsing);
        }
        function test_content_reveal_retains_exit_but_blocks_hidden_input() {
            const button = findChild(gallery, "motion-details-toggle");
            const reveal = findChild(gallery, "motion-details-reveal");
            gallery.detailsOpen = false;
            tryCompare(reveal, "progress", 0, 1000);
            verify(!reveal.visible);
            button.clicked();
            wait(40);
            verify(reveal.progress > 0 && reveal.progress < 1);
            verify(reveal.implicitHeight > 0 && reveal.implicitHeight < reveal.contentHeight);
            const previous = reveal.progress;
            button.clicked();
            compare(reveal.progress, previous); // Retarget, never snap to a new origin.
            verify(reveal.visible && !reveal.children[0].enabled);
            wait(25);
            button.clicked();
            tryCompare(reveal, "progress", 1, 1500);
            compare(reveal.implicitHeight, reveal.contentHeight);
            button.clicked();
            Theme.reducedMotion = true;
            compare(reveal.progress, 0);
            verify(!reveal.visible);
        }
        function test_page_pane_preserves_button_feedback_and_blocks_second_input() {
            const pane = findChild(gallery, "motion-page-transition");
            const button = findChild(gallery, "motion-page-toggle");
            Theme.reducedMotion = true;
            pane.request("overview", false);
            Theme.reducedMotion = false;
            let activations = 0;
            const count = () => activations++;
            button.clicked.connect(count);
            button.forceActiveFocus(Qt.TabFocusReason);
            keyClick(Qt.Key_Return);
            compare(pane.phase, "leaving");
            verify(button.enabled); // Do not replace exit colors with disabled gray.
            wait(20);
            verify(button.transform[0].compression > 0);
            mouseClick(button, button.width / 2, button.height / 2);
            keyClick(Qt.Key_Return);
            compare(pane.requestedKey, "audio");
            compare(activations, 1);
            button.clicked.disconnect(count);
            tryCompare(pane, "phase", "idle", 1000);
            compare(pane.currentKey, "audio");
            verify(button.activeFocus && button.visualFocus);
            pane.request("overview", false);
            wait(20);
            pane.request("audio");
            verify(button.activeFocus && button.visualFocus);
            tryCompare(pane, "phase", "idle", 1000);
            compare(pane.currentKey, "audio");
        }
        function test_page_handoff_latest_intent_and_reversal() {
            Theme.reducedMotion = true;
            pages.request("overview", false);
            Theme.reducedMotion = false;
            const committed = pages.commits;
            pages.request("audio");
            verify(!pages.acceptsInput);
            compare(pages.currentKey, "overview");
            wait(20);
            const opacity = pages.opacity;
            pages.request("overview", false);
            compare(pages.opacity, opacity);
            verify(pages.acceptsInput);
            tryCompare(pages, "phase", "idle", 1000);
            compare(pages.commits, committed); // Cancelled exit never swaps content.
            pages.request("audio");
            pages.request("network");
            tryCompare(pages, "currentKey", "network", 300);
            tryCompare(pages, "phase", "idle", 1000);
            compare(pages.commits, committed + 1);
            verify(!pages.committedWhileVisible);
            compare(pages.opacity, 1);
            compare(pages.offset, 0);
            const completions = pages.completions;
            pages.request("audio");
            wait(15);
            Theme.reducedMotion = true;
            compare(pages.currentKey, "audio");
            compare(pages.phase, "idle");
            compare(pages.opacity, 1);
            compare(pages.offset, 0);
            compare(pages.completions, completions + 1);
            wait(180);
            compare(pages.completions, completions + 1);
        }
        function test_rapid_click_pulse_finishes_and_respects_reduced_motion() {
            pulseExample.pressed = true;
            pulseExample.pressed = false;
            verify(pulseExample.pulse);
            wait(35);
            verify(pulseExample.opacity > 0);
            tryCompare(pulseExample, "pulse", false, 300);
            tryCompare(pulseExample, "opacity", 0, 500);
            pulseExample.pressed = true;
            pulseExample.pressed = false;
            Theme.reducedMotion = true;
            verify(!pulseExample.pulse);
            compare(pulseExample.opacity, 0);
        }
        function test_wallpaper_pointer_does_not_leave_keyboard_outline() {
            tryVerify(() => findChild(wallpapers, "wallpaper-1") !== null, 1500);
            const tile = findChild(wallpapers, "wallpaper-1");
            tile.forceActiveFocus(Qt.TabFocusReason);
            verify(tile.visualFocus);
            mouseClick(tile, tile.width / 2, tile.height / 2);
            verify(!tile.visualFocus);
            tryVerify(() => !wallpapers.busy, 1500);
            verify(!tile.visualFocus);
        }
        function test_spring_parameters_and_completion() {
            for (const name of Object.keys(Material.springs)) {
                const spring = createTemporaryObject(springComponent, null, {
                    motion: name,
                    targetValue: 24
                });
                compare(spring.value, 24); // No unsolicited startup animation.
                verify(!spring.running);
                spring.targetValue = 8;
                verify(spring.running);
                for (let i = 0; i < 300 && spring.running; i++)
                    spring.sample(1 / 144);
                compare(spring.value, 8);
                compare(spring.velocity, 0);
                compare(spring.completions, 1);
                spring.sample(1);
                compare(spring.completions, 1);
            }
        }
        function test_retarget_preserves_position_and_velocity() {
            const spring = createTemporaryObject(springComponent, null);
            spring.targetValue = 100;
            spring.sample(0.05);
            const x = spring.value;
            const v = spring.velocity;
            spring.targetValue = 0;
            compare(spring.value, x);
            compare(spring.velocity, v);
            for (let i = 0; i < 5; i++) {
                spring.sample(0.016);
                spring.targetValue = i % 2 ? 100 : 0;
            }
            spring.targetValue = 12;
            spring.sample(5); // Suspended/delayed frame still finishes correctly.
            compare(spring.value, 12);
            compare(spring.completions, 1);
            verify(!spring.running);
        }
        function test_refresh_rates_and_non_bouncing_effects() {
            const values = [];
            for (const rate of [30, 60, 144]) {
                const spring = createTemporaryObject(springComponent, null, {
                    motion: "fastEffects"
                });
                spring.targetValue = 1;
                let previous = 0;
                for (let i = 0; i < rate / 2; i++) {
                    spring.sample(1 / rate);
                    verify(spring.value >= previous && spring.value <= 1);
                    previous = spring.value;
                }
                values.push(spring.value);
            }
            fuzzyCompare(values[0], values[1], 0.001);
            fuzzyCompare(values[1], values[2], 0.001);
        }
        function test_reduced_motion_during_spring_and_sidebar() {
            const spring = createTemporaryObject(springComponent, null);
            spring.targetValue = 1;
            sidebar.move(true);
            sidebar.sample(0.03);
            Theme.reducedMotion = true;
            compare(spring.value, 1);
            verify(!spring.running);
            compare(spring.completions, 1);
            compare(sidebar.position, 1);
            verify(!sidebar.running);
            const done = sidebar.completions;
            sidebar.move(false);
            compare(sidebar.position, 0);
            compare(sidebar.completions, done + 1);
        }
        function test_popup_reduced_motion_during_exit() {
            popup.present();
            popup.framePresented();
            const done = popup.completions;
            popup.dismiss();
            wait(20);
            Theme.reducedMotion = true;
            tryCompare(popup, "completions", done + 1, 100);
            verify(popup.finishedSent);
            popup.present();
            wait(150);
            compare(popup.completions, done + 1);
            verify(!popup.closing && !popup.finishedSent);
        }
        function test_real_switch_and_slider_hold() {
            toggle.marked = false;
            mousePress(toggle, 30, 24);
            wait(60);
            verify(toggle.down);
            mouseRelease(toggle, 30, 24);
            verify(toggle.marked);
            const thumb = findChild(toggle, "control-switch-thumb");
            tryVerify(() => Math.abs(thumb.width - 24) < 0.01 && Math.abs(thumb.x - 24) < 0.01, 1500);
            mousePress(slider, 200, 24);
            mouseMove(slider, 280, 24, 40);
            verify(slider.pressed);
            verify(slider.value > 65); // No smoothing/delay on the user's value.
            mouseRelease(slider, 280, 24);
            verify(!slider.pressed);
        }
        function test_flat_choice_and_chevron_retarget() {
            compare(choice.background.color, Qt.rgba(0, 0, 0, 0));
            const chevron = findChild(choice, "choice-chevron");
            verify(chevron !== null);
            choice.popup.open();
            wait(40);
            verify(choice.popup.opacity > 0 && choice.popup.opacity < 1);
            const first = choice.popup.contentItem.itemAtIndex(0);
            verify(first !== null);
            verify(first.contentItem.text.includes("Desktop"));
            verify(!first.contentItem.text.includes("undefined"));
            verify(chevron.rotation !== 90 && chevron.rotation !== -90);
            choice.popup.close();
            wait(20);
            verify(choice.popup.visible && choice.popup.dismissing);
            verify(!choice.popup.contentItem.enabled);
            choice.popup.open();
            tryVerify(() => Math.abs(chevron.rotation + 90) < 0.001, 1500);
            choice.popup.close();
            Theme.reducedMotion = true;
            compare(chevron.rotation, 90);
            tryCompare(choice.popup, "visible", false, 100);
        }
        function test_effects_complete_when_reduced_motion_changes() {
            effects.lit = true;
            wait(25);
            verify(effects.opacity > 0 && effects.opacity < 1);
            Theme.reducedMotion = true;
            compare(effects.opacity, 1);
            compare(effects.color, Qt.rgba(1, 1, 1, 1));
            effects.lit = false;
            compare(effects.opacity, 0);
            compare(effects.color, Qt.rgba(0, 0, 0, 1));
        }
        function test_gallery_runs_production_lifecycle() {
            const button = findChild(gallery, "motion-sidebar-toggle");
            const popupButton = findChild(gallery, "motion-popup-toggle");
            verify(button !== null && popupButton !== null);
            const done = gallery.sidebarCompletions;
            button.clicked();
            wait(40);
            button.clicked();
            wait(20);
            button.clicked();
            tryVerify(() => gallery.sidebarCompletions > done, 1000);
            verify(gallery.sidebarOpen);
            popupButton.clicked();
            wait(40);
            const retired = gallery.popupCompletions;
            popupButton.clicked();
            wait(20);
            popupButton.clicked();
            wait(200);
            compare(gallery.popupCompletions, retired);
            popupButton.clicked();
            tryCompare(gallery, "popupCompletions", retired + 1, 1000);
        }
        function test_popup_reopen_and_blocked_cleanup() {
            popup.present();
            popup.framePresented();
            verify(popup.entered);
            const done = popup.completions;
            popup.dismiss();
            wait(25);
            popup.present();
            wait(180);
            compare(popup.completions, done); // Old close cannot destroy reopen.
            popup.canDispose = false;
            popup.dismiss();
            wait(180);
            compare(popup.completions, done);
            popup.canDispose = true;
            compare(popup.completions, done + 1);
            popup.finishIfReady();
            compare(popup.completions, done + 1);
        }
        function cleanupTestCase() {
            console.log("MOTION_TEST_OK");
            Quickshell.quit();
        }
    }
}
