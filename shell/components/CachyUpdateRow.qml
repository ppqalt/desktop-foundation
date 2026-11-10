pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../theme"

ColumnLayout {
    id: root
    objectName: "cachyUpdateRow"
    Layout.maximumWidth: Infinity
    readonly property string stateDir: (Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")) + "/arch-update/"
    property var packages: null
    property var aur: null
    property var flatpak: null
    property string checkedAt: ""
    property bool expanded: false
    readonly property int total: (packages?.length ?? 0) + (aur?.length ?? 0) + (flatpak?.length ?? 0)
    readonly property string status: check.running ? "Checking…" : !checkedAt || packages === null ? "No completed update check" : aur === null || flatpak === null ? "Last check: " + packages.length + " repository updates" : "Last check: " + total + (total === 1 ? " update" : " updates")
    required property string backend
    property string checkError: ""
    readonly property bool busy: check.running || updater.running
    signal settled
    spacing: 0
    ControlRequest {
        id: check
        onFinished: code => {
            root.checkError = code === 0 ? "" : failure || "Update check failed. Try again.";
            root.settled();
        }
    }
    ControlRequest {
        id: updater
        // Existing terminal launcher exits after handing Kitty the command;
        // it deliberately has no JSON response body.
        expectJson: false
        onFinished: code => {
            root.checkError = code === 0 ? "" : failure || "Could not open Cachy Update.";
            root.settled();
        }
    }
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
    GridLayout {
        Layout.fillWidth: true
        Layout.maximumWidth: Infinity
        columns: root.width >= 560 ? 4 : 3
        columnSpacing: 4
        rowSpacing: 0
        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.columnSpan: root.width >= 560 ? 1 : 3
            spacing: 0
            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: "Cachy Update"
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyLarge
            }
            Text {
                objectName: "updateStatus"
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.status
                color: Material.colors.on_surface_variant
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
            }
        }
        ControlAction {
            objectName: "updateDetails"
            compact: true
            text: root.expanded ? "Hide" : "Details"
            Accessible.name: root.expanded ? "Hide update details" : "Show update details"
            Layout.preferredWidth: 96
            Layout.maximumWidth: 96
            onClicked: root.expanded = !root.expanded
        }
        ControlAction {
            objectName: "cachyUpdateShortcut"
            compact: true
            text: "Check"
            Accessible.name: "Check available updates"
            Layout.preferredWidth: 96
            Layout.maximumWidth: 96
            enabled: !root.busy
            onClicked: {
                root.checkError = "";
                check.command = [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "check-updates"];
                check.running = true;
            }
        }
        ControlAction {
            objectName: "cachyUpdateLaunch"
            compact: true
            text: "Update"
            Accessible.name: "Open Cachy Update"
            Layout.preferredWidth: 96
            Layout.maximumWidth: 96
            enabled: !root.busy
            onClicked: {
                root.checkError = "";
                updater.command = [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "control-center", "cachy-update"];
                updater.running = true;
            }
        }
    }
    WidgetStatus {
        Layout.fillWidth: true
        message: root.checkError
        font.pixelSize: Material.type.bodyMedium
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
