pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Io

Variants {
    id: root
    property var settings: JSON.parse(manifest.text() || "{}")
    property int startupRetries: 0
    property var manifest: FileView {
        path: (Quickshell.env("XDG_CACHE_HOME") || Quickshell.env("HOME") + "/.cache") + "/desktop-foundation/overview/backdrop.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
    }
    // Wallpaper/cache preparation can overlap shell startup. Stop after 5s;
    // normal updates use the file watcher, never overview polling.
    property Timer startupRetry: Timer {
        interval: 250
        repeat: true
        running: !root.settings.image && root.startupRetries < 20
        onTriggered: {
            root.startupRetries++;
            root.manifest.reload();
        }
    }
    model: Quickshell.screens
    // qmllint disable uncreatable-type
    PanelWindow {
        required property var modelData
        screen: modelData
        color: "#26000000"
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        mask: Region {}
        WlrLayershell.namespace: "desktop-foundation-overview-backdrop"
        WlrLayershell.layer: WlrLayer.Background
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        Image {
            anchors.fill: parent
            source: root.settings.image ?? ""
            fillMode: root.settings.mode === "fit" ? Image.PreserveAspectFit : root.settings.mode === "center" ? Image.Pad : root.settings.mode === "tile" ? Image.Tile : Image.PreserveAspectCrop
        }
        Rectangle {
            anchors.fill: parent
            color: "#26000000"
        }
    }
}
