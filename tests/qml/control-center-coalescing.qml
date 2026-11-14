import QtQuick
import QtTest
import Quickshell
import Quickshell.Io
import "shell/services"

ShellRoot {
    ControlCenterState {
        id: memory
    }
    FileView {
        id: calls
        path: Quickshell.env("DF_CONTROL_CALLS")
        printErrors: false
        blockLoading: true
    }
    TestCase {
        name: "SnapshotWriteCoalescing"
        when: true
        function writes() {
            calls.reload();
            return calls.text().split("\n").filter(line => line === "snapshot-cache write").length;
        }
        function test_bursts_keep_immediate_readback_and_persist_latest() {
            const baseline = Quickshell.env("DF_CONTROL_COALESCE_BASELINE") === "1";
            // Deliberately yield between publications to exercise completed
            // child writers, not merely synchronous JavaScript deduplication.
            for (let value = 1; value <= 20; value++) {
                memory.publish({
                    network: {
                        available: true,
                        enabled: value % 2 === 0,
                        wifi: true,
                        devices: [],
                        connections: []
                    }
                });
                compare(memory.current.network.enabled, value % 2 === 0);
                wait(20);
            }
            tryVerify(() => !memory.busy, 1500);
            const count = writes();
            console.log("COALESCE_WRITES " + count);
            if (baseline)
                verify(count > 5, "baseline must exercise repeated subprocess writes");
            else
                verify(count <= 3 && count >= 1, "bursts must coalesce without starvation");
            compare(memory.saved, JSON.stringify(memory.snapshot));
            for (let i = 0; i < 10; i++)
                memory.publish({
                    network: memory.current.network
                });
            tryVerify(() => !memory.busy, 1000);
            compare(writes(), count, "unchanged snapshots must not spawn another writer");
        }
        function cleanupTestCase() {
            console.log('CONTROL_RESULT {"failures":0}');
            Quickshell.quit();
        }
    }
}
