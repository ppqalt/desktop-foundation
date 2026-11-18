pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import "../components"

// One bounded startup read; reloadConfiguration reloads this with the shell.
QtObject {
    id: root
    property var settings: ({
            media: {
                home_player: "spotify"
            },
            screen_corners: {
                enabled: true,
                radius: 28
            }
        })
    readonly property string error: request.phase === "error" ? request.failure : ""
    function refresh(): void {
        if (!request.running)
            request.running = true;
    }
    property ControlRequest request: ControlRequest {
        command: [Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/foundation", "config", "desktop"]
        running: true
        onFinished: code => {
            if (code === 0)
                root.settings = response.settings;
            else
                console.warn("Desktop settings: " + failure);
        }
    }
}
