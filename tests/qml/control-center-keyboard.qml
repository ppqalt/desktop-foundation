import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "shell/surfaces"

ShellRoot {
    id: fixture
    property int failures: 0
    function check(ok, why) {
        if (!ok) {
            failures++;
            console.error("CONTROL_FAIL " + why);
        }
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
    ControlCenterFixture {
        id: panel
        width: 720
        height: 300
        minimumHeight: 300
        maximumHeight: 300
        minimumWidth: 720
        maximumWidth: 720
        sidebarStyle: ({
                width: 720,
                padding: 12,
                radius: 14,
                opacity: 1,
                openingMs: 50,
                closingMs: 50
            })
        TestCase {
            id: keys
            name: "ControlCenterKeyboard"
            optional: true
            when: false
        }
        function key(code, modifiers = Qt.NoModifier) {
            keys.keyClick(code, modifiers);
            keys.wait(10);
        }
        function settle() {
            keys.tryVerify(() => !panel.snapshot().pageMoving, 1000);
            keys.wait(30);
        }
        Timer {
            interval: 40
            running: true
            repeat: true
            onTriggered: {
                if (!panel.audioFresh || !panel.networkFresh || panel.snapshot().moving)
                    return;
                stop();
                fixture.check(panel.height === 300, "short-window fixture retains its requested height");
                const mute = find(panel.contentItem, "outputMute");
                const slider = find(panel.contentItem, "outputVolume");
                // Simulate an empty/disabled first-open focus owner before a
                // device snapshot arrives. An arrow must enter usable content.
                const scope = find(panel.contentItem, "controlCenterFocusScope");
                if (scope) {
                    panel.activeFocusItem.focus = false;
                    scope.forceActiveFocus();
                    panel.key(Qt.Key_Down);
                    fixture.check(panel.activeFocusItem !== scope && panel.activeFocusItem.activeFocusOnTab, "arrows leave the empty focus owner");
                } else {
                    fixture.check(false, "keyboard focus owner is not testable");
                }
                fixture.check(find(panel.contentItem, "networkToggle").activeFocus, "first arrow selects first available control");
                panel.key(Qt.Key_D);
                const networkDetails = find(panel.contentItem, "networkDetails");
                fixture.check(networkDetails.activeFocus, "D navigates to network details");
                panel.key(Qt.Key_Return);
                panel.settle();
                panel.key(Qt.Key_S);
                fixture.check(find(panel.contentItem, "networkPageToggle").activeFocus, "S enters network controls");
                panel.key(Qt.Key_Down);
                fixture.check(find(panel.contentItem, "networkDevice-enp4s0").activeFocus, "Down skips unavailable Wi-Fi");
                panel.key(Qt.Key_Tab, Qt.ShiftModifier);
                fixture.check(find(panel.contentItem, "networkPageToggle").activeFocus, "Shift+Tab skips unavailable Wi-Fi");
                panel.key(Qt.Key_Escape);
                panel.settle();
                fixture.check(networkDetails.activeFocus && networkDetails.visualFocus, "network back restores invoking keyboard action");
                const visited = [];
                find(panel.contentItem, "networkToggle").forceActiveFocus(Qt.TabFocusReason);
                for (let step = 0; step < 24; step++) {
                    const focused = panel.activeFocusItem;
                    fixture.check(focused && focused.enabled && focused.visible, "Tab never lands on hidden or disabled controls");
                    if (focused?.objectName)
                        visited.push(focused.objectName);
                    panel.key(Qt.Key_Tab);
                }
                for (const name of ["networkToggle", "networkDetails", "audioDetails", "outputMute", "outputVolume", "applicationsToggle", "updateDetails", "cachyUpdateShortcut", "cachyUpdateLaunch"])
                    fixture.check(visited.includes(name), "Tab reaches " + name);
                const apps = find(panel.contentItem, "applicationsToggle");
                // First device readback must not steal a selection already made
                // with Tab/arrows while the snapshot was being fetched.
                panel.initialFocus = true;
                apps.forceActiveFocus(Qt.TabFocusReason);
                keys.wait(20);
                const response = JSON.parse(JSON.stringify(panel.stateData));
                panel.acceptSnapshot(response, panel.requestRevision);
                keys.wait(20);
                fixture.check(apps.activeFocus, "snapshot must not steal keyboard focus");
                apps.forceActiveFocus(Qt.TabFocusReason);
                panel.key(Qt.Key_Return);
                panel.settle();
                fixture.check(panel.page === "audio", "Enter opens Audio");
                const back = find(panel.contentItem, "backAction");
                fixture.check(back.activeFocus && back.visualFocus, "keyboard page entry keeps a visible focus indicator");
                panel.key(Qt.Key_Down);
                const choice = find(panel.contentItem, "outputChoice");
                fixture.check(choice.activeFocus, "Down reaches the output selector");
                panel.key(Qt.Key_Return);
                fixture.check(choice.popup.visible, "Enter opens output dropdown");
                panel.key(Qt.Key_Down);
                panel.key(Qt.Key_Escape);
                keys.tryCompare(choice.popup, "visible", false, 1000);
                fixture.check(panel.page === "audio" && !panel.closing, "Escape closes dropdown before page");
                slider.forceActiveFocus(Qt.TabFocusReason);
                const previous = slider.value;
                panel.key(Qt.Key_Right);
                fixture.check(slider.value > previous && slider.activeFocus, "Right adjusts slider without moving focus");
                panel.key(Qt.Key_Down);
                fixture.check(!slider.activeFocus, "Down exits slider");
                // Tab must reveal the actual focused control in a short panel.
                find(panel.contentItem, "applicationVolume-100").forceActiveFocus(Qt.TabFocusReason);
                keys.wait(30);
                const viewport = find(panel.contentItem, "controlCenterScroll");
                fixture.check(viewport.contentY > 0, "focused lower control scrolls into view");
                panel.key(Qt.Key_Escape);
                panel.settle();
                fixture.check(panel.page === "overview" && apps.activeFocus && apps.visualFocus, "Back restores keyboard focus to invoking action");
                panel.audioFresh = false;
                panel.networkFresh = false;
                panel.activeFocusItem.focus = false;
                scope.forceActiveFocus();
                panel.key(Qt.Key_Down);
                fixture.check(find(panel.contentItem, "audioDetails").activeFocus, "missing device state still permits keyboard entry");
                apps.enabled = false;
                panel.navigate("audio", apps);
                panel.settle();
                panel.key(Qt.Key_Escape);
                panel.settle();
                fixture.check(!apps.activeFocus && panel.pageEntryControl().activeFocus && panel.pageEntryControl().enabled, "disabled return bookmark falls back to an available control");
                apps.enabled = true;
                panel.key(Qt.Key_Escape);
                fixture.check(panel.closing, "Escape closes overview");
                console.log("CONTROL_RESULT " + JSON.stringify({
                    failures: fixture.failures
                }));
                Qt.quit();
            }
        }
    }
    Timer {
        interval: 8000
        running: true
        onTriggered: {
            console.error("CONTROL_TIMEOUT");
            Qt.quit();
        }
    }
}
