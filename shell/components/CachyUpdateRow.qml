pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../theme"

ColumnLayout {
    id: root
    readonly property string stateDir: (Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")) + "/arch-update/"
    property var packages: null
    property var aur: null
    property var flatpak: null
    property string checkedAt: ""
    property bool expanded: false
    readonly property int total: (packages?.length ?? 0) + (aur?.length ?? 0) + (flatpak?.length ?? 0)
    readonly property string status: !checkedAt || packages === null ? "No completed update check" : aur === null || flatpak === null ? "Last check: " + packages.length + " repository updates" : "Last check: " + total + (total === 1 ? " update" : " updates")
    signal launchRequested
    spacing: 0
    function lines(value): var {
        return value.split("\n").map(s => s.trim()).filter(s => s.length > 0);
    }
    FileView {
        path: root.stateDir + "last_updates_check_packages"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.packages = root.lines(text())
        onLoadFailed: root.packages = null
    }
    FileView {
        path: root.stateDir + "last_updates_check_aur"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.aur = root.lines(text())
        onLoadFailed: root.aur = null
    }
    FileView {
        path: root.stateDir + "last_updates_check_flatpak"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.flatpak = root.lines(text())
        onLoadFailed: root.flatpak = null
    }
    FileView {
        path: root.stateDir + "last_updates_check_time"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            const values = root.lines(text());
            root.checkedAt = values.length > 1 ? values[1] : "";
        }
        onLoadFailed: root.checkedAt = ""
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 4
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0
            Text {
                text: "Cachy Update"
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyLarge
            }
            Text {
                objectName: "updateStatus"
                text: root.status
                color: Material.colors.on_surface_variant
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
            }
        }
        ControlAction {
            objectName: "updateDetails"
            compact: true
            iconOnly: true
            symbol: root.expanded ? "expand_less" : "expand_more"
            text: root.expanded ? "Hide update details" : "Show update details"
            Layout.preferredWidth: 48
            onClicked: root.expanded = !root.expanded
        }
        ControlAction {
            objectName: "cachyUpdateShortcut"
            compact: true
            iconOnly: true
            symbol: "system_update_alt"
            text: "Open Cachy Update"
            Layout.preferredWidth: 48
            onClicked: root.launchRequested()
        }
    }
    ColumnLayout {
        visible: root.expanded
        Layout.fillWidth: true
        spacing: 4
        Text {
            Layout.fillWidth: true
            text: root.checkedAt ? "Checked " + root.checkedAt : "Run a check in Cachy Update to see details."
            color: Material.colors.on_surface_variant
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
            wrapMode: Text.Wrap
        }
        Text {
            Layout.fillWidth: true
            visible: root.checkedAt !== ""
            text: "Repository: " + (root.packages?.length ?? "—") + "  ·  AUR: " + (root.aur?.length ?? "—") + "  ·  Flatpak: " + (root.flatpak?.length ?? "—")
            color: Material.colors.on_surface_variant
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
            wrapMode: Text.Wrap
        }
        Text {
            objectName: "updateList"
            Layout.fillWidth: true
            visible: root.total > 0 && root.checkedAt !== ""
            text: [...(root.packages ?? []), ...(root.aur ?? []), ...(root.flatpak ?? [])].slice(0, 12).join("\n") + (root.total > 12 ? "\n…" : "")
            textFormat: Text.PlainText
            color: Material.colors.on_surface_variant
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
            wrapMode: Text.WrapAnywhere
        }
    }
}
