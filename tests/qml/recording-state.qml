import QtQuick
import QtTest
import Quickshell
import "services"
import "components"

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
    TestCase {
        name: "RecordingState"
        when: true
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
        }
        function cleanupTestCase() {
            console.log("RECORDING_STATE_PASS");
            Quickshell.quit();
        }
    }
}
