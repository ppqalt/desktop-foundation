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
                root.find(panel.contentItem, "applicationsToggle").clicked();
            }
        }
    }
    Timer {
        interval: 1800
        running: true
        onTriggered: {
            let target = null;
            let action = "click";
            if (root.page === "replay") {
                const editor = root.find(panel.contentItem, "replayEditor");
                if (editor.session) {
                    editor.select(editor.session.duration * .25, editor.session.duration * .8);
                    target = root.find(editor, "replayFootage");
                }
            } else if (root.page === "check" || root.page === "check-error")
                target = root.find(panel.contentItem, "cachyUpdateShortcut");
            else if (root.page === "scroll") {
                target = root.find(panel.contentItem, "controlCenterScroll");
                action = "wheel";
            }
            if (target) {
                const point = target.mapToGlobal(target.width / 2, target.height / 2);
                console.log("PRIVATE_INPUT " + JSON.stringify({
                    action: action,
                    x: point.x,
                    y: point.y
                }));
            }
        }
    }

    Timer {
        interval: 3500
        running: true
        onTriggered: {
            if (root.page === "scroll" && root.find(panel.contentItem, "controlCenterScroll").contentY <= 0)
                console.error("VISUAL_SCROLL_FAILED");
            if (root.page === "replay" && !root.find(panel.contentItem, "replayFootage").activeFocus)
                console.error("VISUAL_TIMELINE_FOCUS_FAILED");
            panel.contentItem.children[0].grabToImage(result => {
                if (!result.saveToFile(Quickshell.env("DF_VISUAL_CAPTURE")))
                    console.error("VISUAL_CAPTURE_FAILED");
                console.log("VISUAL_READY");
                Qt.quit();
            });
        }
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
