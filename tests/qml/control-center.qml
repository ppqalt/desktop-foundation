import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import Quickshell.Io
import "shell/surfaces"

ShellRoot {
    id: fixture
    property int stage: 0
    property int failures: 0
    property bool enabled: true
    property bool concurrentSent: false
    property bool updateCacheChecked: false
    property bool updateLaunchChecked: false
    property bool editorChecked: false
    Process {
        id: updateWriter
        command: ["/bin/sh", "-c", "printf 'repo-three 1 -> 2\\n' >> \"$XDG_STATE_HOME/arch-update/last_updates_check_packages\""]
    }
    property int count: 0
    property real velocity: 0
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
            const found = find(child, name);
            if (found)
                return found;
        }
        return null;
    }
    readonly property var panel: loader.item
    LazyLoader {
        id: loader
        active: fixture.enabled
        ControlCenterFixture {
            recording: ({
                    prepared: true,
                    bufferSeconds: 300,
                    state: {
                        phase: "replay",
                        replay: true,
                        replay_status: "buffering"
                    }
                })
            width: 720
            height: 1000
            sidebarStyle: ({
                    width: 720,
                    padding: 12,
                    radius: 14,
                    opacity: 0.75,
                    openingMs: 240,
                    closingMs: 180
                })
            onDismissed: fixture.enabled = false
            TestCase {
                id: keys
                name: "ControlKeys"
                optional: true
                when: false
            }
            function testApplicationDrag(application) {
                keys.wait(50); // Allow the newly opened Audio page to lay out.
                fixture.check(application.width > 100, "application drag fixture has no usable width");
                keys.mousePress(application, application.width * .4, application.height / 2);
                fixture.check(application.pressed, "application slider did not retain initial press");
                const updated = JSON.parse(JSON.stringify(panel.stateData));
                updated.audio.applications[0].volume = 5;
                panel.stateData = updated;
                panel.syncApplications();
                fixture.check(find(panel.contentItem, "applicationVolume-100") === application, "audio readback recreated the held application slider");
                fixture.check(application.pressed, "audio readback cancelled the drag");
                const reordered = JSON.parse(JSON.stringify(updated));
                reordered.audio.applications.unshift({
                    id: 43,
                    serial: "101",
                    name: "Other player",
                    volume: 25
                });
                panel.stateData = reordered;
                panel.syncApplications();
                fixture.check(find(panel.contentItem, "applicationVolume-100") === application && application.pressed, "stream insertion replaced the held slider");
                const heldValue = application.value;
                fixture.check(heldValue !== 5, "audio readback replaced the held pointer value");
                keys.mouseMove(application, application.width * .7, application.height / 2, 20);
                fixture.check(application.value > heldValue, "held application slider stopped following the pointer");
                keys.mouseRelease(application, application.width * .7, application.height / 2);
                fixture.check(!application.pressed, "application slider did not release");
                panel.stateData = updated;
                panel.syncApplications();
            }
            function key(key, modifiers) {
                keys.keyClick(key, modifiers || Qt.NoModifier);
            }
        }
    }
    Timer {
        id: test
        interval: 30
        running: true
        repeat: true
        onTriggered: {
            if (fixture.stage === 0) {
                if (panel.snapshot().progress > 0 && panel.snapshot().progress < 1) {
                    fixture.check(panel.width === 720 && panel.height === 1000, "sidebar dimensions changed");
                    fixture.stage = 1;
                }
            } else if (fixture.stage === 1) {
                if (panel.snapshot().moving || !panel.stateData.audio)
                    return;
                const replayToggle = find(panel.contentItem, "replayToggle");
                const updateAction = find(panel.contentItem, "cachyUpdateShortcut");
                if (replayToggle && updateAction) {
                    const save = find(panel.contentItem, "saveReplay");
                    const toggleX = replayToggle.mapToItem(panel.contentItem, 0, 0).x;
                    const saveRight = save.mapToItem(panel.contentItem, 0, 0).x + save.width;
                    fixture.check(toggleX > panel.width - 120, "replay toggle is not right-aligned");
                    fixture.check(toggleX >= saveRight && toggleX - saveRight <= 8, "save is not immediately left of the toggle");
                    const launch = find(panel.contentItem, "cachyUpdateLaunch");
                    const updateRight = launch.mapToItem(panel.contentItem, launch.width, 0).x;
                    fixture.check(updateAction.mapToItem(panel.contentItem, 0, 0).x < launch.mapToItem(panel.contentItem, 0, 0).x, "Check is not before Update");
                    fixture.check(Math.abs(updateRight - (panel.width - panel.sidebarStyle.padding - 16)) <= 1, "update check is not aligned with the section inset");
                } else {
                    fixture.check(false, "sidebar actions missing");
                }
                const updateStatus = find(panel.contentItem, "updateStatus");
                if (!updateStatus)
                    return;
                if (!fixture.updateCacheChecked) {
                    if (updateStatus.text !== "Last check: 3 updates")
                        return;
                    const updateDetails = find(panel.contentItem, "updateDetails");
                    updateDetails.clicked();
                    fixture.check(find(panel.contentItem, "updateList").visible, "update details did not expand");
                    fixture.check(find(panel.contentItem, "updateList").text.includes("aur-one"), "cached update details missing");
                    updateDetails.clicked();
                    fixture.check(!find(panel.contentItem, "updateList").visible, "update details did not collapse");
                    updateAction.clicked();
                    fixture.updateCacheChecked = true;
                    updateWriter.running = true;
                    return;
                }
                if (updateStatus.text !== "Last check: 4 updates")
                    return;
                if (!fixture.editorChecked) {
                    find(panel.contentItem, "saveReplay").clicked();
                    fixture.check(panel.page === "replay", "Save Replay did not open the editor");
                    fixture.editorChecked = true;
                    return;
                }
                if (panel.page === "replay") {
                    if (panel.busy)
                        return;
                    const editor = find(panel.contentItem, "replayEditor");
                    fixture.check(editor.session && editor.session.duration === 300, "editor did not load its protected source");
                    fixture.check(editor.endTime - editor.startTime === 47, "editor selection changed buffer duration");
                    find(panel.contentItem, "backAction").clicked();
                    fixture.check(find(panel.contentItem, "saveReplay").activeFocus, "return from editor did not restore Save focus");
                }
                const updateRow = find(panel.contentItem, "cachyUpdateRow");
                if (!fixture.updateLaunchChecked) {
                    if (updateRow.busy)
                        return;
                    find(panel.contentItem, "cachyUpdateLaunch").clicked();
                    fixture.updateLaunchChecked = true;
                    return;
                }
                if (updateRow.busy)
                    return;
                fixture.check(updateRow.checkError === "", "empty terminal-launch response reported an update error");
                const savedState = panel.stateData;
                fixture.check(!panel.acceptSnapshot({
                    audio: {},
                    network: {}
                }, panel.requestRevision - 1), "obsolete snapshot accepted");
                fixture.check(panel.stateData === savedState, "obsolete snapshot changed state");
                find(panel.contentItem, "applicationsToggle").clicked();
                fixture.check(panel.page === "audio", "application volumes did not open Audio settings");
                const application = find(panel.contentItem, "applicationVolume-100");
                fixture.check(!!application, "application slider is missing");
                if (application) {
                    test.stop();
                    panel.testApplicationDrag(application);
                    test.start();
                    application.value = 37;
                    application.moved();
                }
                find(panel.contentItem, "backAction").clicked();
                fixture.check(find(panel.contentItem, "applicationsToggle").activeFocus, "return from Audio did not restore application volumes focus");
                panel.requestActivate();
                panel.navigate("audio");
                find(panel.contentItem, "outputChoice").forceActiveFocus();
                fixture.check(find(panel.contentItem, "outputChoice").activeFocus, "output choice not focusable");
                const card = panel.contentItem.children.find(c => c.topRightRadius === 14);
                fixture.check(card && card.bottomRightRadius === 14 && card.topLeftRadius === 0 && Math.abs(card.color.a - .75) < .01, "mirrored glass/corners differ");
                panel.key(Qt.Key_Tab);
                fixture.check(panel.activeFocusItem.Accessible.name.indexOf("Volume") >= 0, "Tab misses output mute");
                panel.key(Qt.Key_Tab);
                const slider = find(panel.contentItem, "outputVolume");
                fixture.check(slider.activeFocus, "Tab misses output volume");
                panel.key(Qt.Key_Right);
                fixture.check(slider.value === 51, "volume does not update immediately");
                panel.key(Qt.Key_A);
                fixture.check(slider.value === 50, "A does not decrease volume immediately");
                panel.key(Qt.Key_D);
                fixture.check(slider.value === 51, "D does not increase volume immediately");
                panel.key(Qt.Key_W);
                fixture.check(find(panel.contentItem, "outputChoice").activeFocus, "W does not navigate upward from slider");
                panel.key(Qt.Key_S);
                fixture.check(slider.activeFocus, "S does not navigate down to slider");
                fixture.stage = 2;
            } else if (fixture.stage === 2) {
                if (panel.busy || panel.queue.length)
                    return;
                if (!fixture.concurrentSent) {
                    fixture.concurrentSent = true;
                    panel.enqueue(["wifi", "on"]);
                    panel.enqueue(["volume", "output", "52"]);
                    return;
                }
                panel.dismiss();
                fixture.stage = 3;
            } else if (fixture.stage === 3) {
                if (!panel || panel.snapshot().progress === 0) {
                    fixture.check(false, "exit finished too early");
                    fixture.stage = 7;
                    return;
                }
                fixture.check(panel.closing && panel.snapshot().moving, "exit has no animation");
                const motion = panel.contentItem; // Reverse the real production controller.
                panel.present();
                fixture.stage = 4;
            } else if (fixture.stage === 4) {
                if (panel.snapshot().moving)
                    return;
                fixture.check(!panel.closing && panel.snapshot().progress === 1, "reversal did not settle open");
                panel.navigate("overview");
                fixture.stage = 8;
                captureDelay.restart();
            } else if (fixture.stage === 9) {
                fixture.stage = 5;
                const update = find(panel.contentItem, "cachyUpdateShortcut");
                update.forceActiveFocus();
                fixture.check(update.activeFocus, "Cachy Update is not keyboard reachable");
                const details = find(panel.contentItem, "audioDetails");
                details.forceActiveFocus();
                panel.key(Qt.Key_D);
                fixture.check(find(panel.contentItem, "inputDetails").activeFocus, "D does not navigate across audio cards");
                panel.key(Qt.Key_A);
                fixture.check(details.activeFocus, "A does not navigate back across audio cards");
                panel.key(Qt.Key_Return);
                fixture.check(panel.page === "audio", "Enter does not open audio details");
                panel.key(Qt.Key_Escape);
                fixture.check(panel.page === "overview" && details.activeFocus, "Back does not restore initiating focus");
                // Walk the overview focus chain, including both connectivity modules.
                let sawNetwork = false, sawMic = false;
                for (let i = 0; i < 30; i++) {
                    panel.key(Qt.Key_Tab);
                    const label = panel.activeFocusItem?.Accessible.name || "";
                    if (!sawNetwork && label.indexOf("Networking") >= 0)
                        panel.key(Qt.Key_Return);
                    sawNetwork = sawNetwork || label.indexOf("Networking") >= 0;
                    sawMic = sawMic || label.indexOf("Microphone") >= 0;
                }
                fixture.check(sawNetwork && sawMic, "keyboard cannot reach microphone/network controls");
                panel.navigate("audio");
                const choice = find(panel.contentItem, "outputChoice");
                choice.forceActiveFocus();
                panel.key(Qt.Key_Space);
                fixture.check(choice.popup.visible, "keyboard cannot open device dropdown");
                panel.key(Qt.Key_Escape);
                fixture.check(!choice.popup.visible && !panel.closing, "Escape does not close dropdown first");
                panel.key(Qt.Key_Space);
                panel.key(Qt.Key_S);
                panel.key(Qt.Key_Space);
                fixture.check(!choice.popup.visible, "Enter does not accept the device choice");
                panel.key(Qt.Key_Escape);
                fixture.check(panel.page === "overview" && !panel.closing, "Escape does not return to overview");
                panel.key(Qt.Key_Escape);
            } else if (fixture.stage === 5) {
                if (panel)
                    return;
                fixture.stage = 6;
                idle.restart();
                test.stop();
            }
        }
    }
    Timer {
        id: captureDelay
        interval: 80
        onTriggered: {
            const directory = Quickshell.env("DF_CONTROL_CAPTURE");
            if (directory)
                panel.contentItem.grabToImage(result => {
                    result.saveToFile(directory);
                    fixture.stage = 9;
                });
            else
                fixture.stage = 9;
        }
    }
    Timer {
        id: idle
        interval: 2500
        onTriggered: {
            fixture.check(!fixture.panel, "closed surface remained resident");
            console.log("CONTROL_RESULT " + JSON.stringify({
                failures: fixture.failures
            }));
            Qt.quit();
        }
    }
    Timer {
        interval: 6500
        running: true
        onTriggered: {
            console.error("CONTROL_TIMEOUT stage=" + fixture.stage);
            Qt.quit();
        }
    }
}
