pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import "shell/surfaces"

// Loaded only by the private visual harness with inert system workers.
ShellRoot {
    id: root
    property string page: Quickshell.env("DF_VISUAL_PAGE") || "overview"
    property string replayState: Quickshell.env("DF_VISUAL_STATE") || "ready"
    Component.onCompleted: Quickshell.watchFiles = false
    function find(item, name): var {
        if (item.objectName === name)
            return item;
        for (const child of item.children || []) {
            const found = find(child, name);
            if (found)
                return found;
        }
        return null;
    }
    ControlCenterFixture {
        id: panel
        targetScreen: Quickshell.screens[0]
        sidebarStyle: ({
                width: Number(Quickshell.env("DF_VISUAL_WIDTH")) || 720,
                padding: 12,
                radius: 14,
                opacity: 0.75,
                openingMs: 240,
                closingMs: 180
            })
        recording: ({
                prepared: true,
                bufferSeconds: 300,
                state: {
                    phase: root.replayState === "error" ? "error" : root.replayState === "off" ? "idle" : "replay",
                    replay: !["off", "error"].includes(root.replayState),
                    replay_status: root.replayState
                }
            })
    }
    Timer {
        interval: 800
        running: true
        onTriggered: {
            if (root.page === "replay") {
                panel.navigate("replay");
                root.find(panel.contentItem, "replayEditor").open();
            } else if (root.page === "audio") {
                panel.navigate("audio");
                panel.applicationsExpanded = true;
            }
        }
    }
    Timer {
        interval: 3500
        running: true
        onTriggered: panel.contentItem.children[0].grabToImage(result => {
            if (!result.saveToFile(Quickshell.env("DF_VISUAL_CAPTURE")))
                console.error("VISUAL_CAPTURE_FAILED");
            console.log("VISUAL_READY");
            Qt.quit();
        })
    }
    Timer {
        interval: 15000
        running: true
        onTriggered: {
            console.error("VISUAL_TIMEOUT");
            Qt.quit();
        }
    }
}
