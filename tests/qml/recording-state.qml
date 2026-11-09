import QtQuick
import QtTest
import Quickshell
import "components"
import "services"

ShellRoot {
    RecordingState {
        id: recording
    }

    ClockVisibility {
        id: clock

        recordingPhase: recording.phase
    }

    ControlRequest {
        id: writer
    }

    QtObject {
        id: mockRecording

        property bool prepared: true
        property int bufferSeconds: 135
        property var state: ({
                "phase": "idle"
            })
    }

    Component {
        id: replayComponent

        ReplayControl {
            recording: mockRecording
        }
    }

    TestCase {
        function writePhase(phase) {
            writer.command = [Quickshell.shellDir + "/../scripts/record", "fixture-write", phase];
            writer.running = true;
            tryCompare(writer, "running", false, 1500);
            compare(writer.phase, "ready");
            tryCompare(recording, "phase", phase, 1500);
        }

        function test_atomic_state_changes_and_clock_override() {
            tryCompare(recording, "prepared", true, 1500);
            compare(recording.phase, "idle");
            verify(!clock.visible);
            writePhase("starting");
            verify(clock.visible);
            writePhase("recording");
            clock.toggle();
            verify(!clock.visible);
            writePhase("stopping");
            verify(!clock.visible);
            clock.toggle();
            verify(clock.visible);
            wait(100);
            compare(recording.phase, "stopping");
            verify(clock.visible);
            writePhase("error");
            verify(clock.visible);
            clock.toggle();
            verify(!clock.visible);
            writePhase("idle");
            verify(!clock.visible);
            writePhase("starting");
            verify(clock.visible);
            writePhase("idle");
            verify(!clock.visible);
            writePhase("replay");
            verify(recording.replayActive);
            verify(clock.visible);
            writePhase("replay-recording");
            verify(recording.replayActive);
            verify(clock.visible);
            writePhase("idle");
            verify(!recording.replayActive);
            verify(!clock.visible);
        }

        function test_replay_control_uses_confirmed_state() {
            const control = createTemporaryObject(replayComponent, null);
            verify(control !== null);
            compare(control.duration, "2:15");
            compare(control.statusText, "Off");
            let requested = "";
            control.setEnabled.connect(enabled => {
                return requested = enabled ? "on" : "off";
            });
            const toggle = findChild(control, "replayToggle");
            const save = findChild(control, "saveReplay");
            toggle.clicked();
            compare(requested, "on");
            compare(control.statusText, "Off");
            mockRecording.state = {
                "phase": "starting",
                "replay": true,
                "replay_status": "starting"
            };
            compare(control.statusText, "Starting");
            verify(toggle.marked);
            mockRecording.state = {
                "phase": "replay",
                "replay": true,
                "replay_status": "buffering"
            };
            compare(control.statusText, "Buffering");
            verify(save.enabled);
            let saves = 0;
            control.saveRequested.connect(() => {
                return saves++;
            });
            save.clicked();
            compare(saves, 1);
            verify(!findChild(control, "replayStatus").text.includes("Saved"));
            mockRecording.state = {
                "phase": "replay-recording",
                "replay": true,
                "replay_status": "ready",
                "save_status": "saved"
            };
            compare(control.statusText, "Ready");
            verify(findChild(control, "replayStatus").text.includes("Saved"));
            mockRecording.state = {
                "phase": "recording",
                "replay": true,
                "replay_status": "paused"
            };
            compare(control.statusText, "Paused");
            verify(!save.enabled);
            toggle.clicked();
            compare(requested, "off");
            mockRecording.state = {
                "phase": "error",
                "error": "failed"
            };
            compare(control.statusText, "Error");
            verify(!toggle.marked);
            verify(save.enabled); // Retained footage remains accessible after capture failure.
            mockRecording.bufferSeconds = 0;
            compare(control.duration, "");
            compare(findChild(control, "replayStatus").text, "Error");
            mockRecording.bufferSeconds = 135;
        }

        function cleanupTestCase() {
            console.log("RECORDING_STATE_PASS");
            Quickshell.quit();
        }

        name: "RecordingState"
        when: true
    }
}
