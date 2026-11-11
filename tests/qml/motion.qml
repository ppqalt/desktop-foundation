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
        height: 320
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
