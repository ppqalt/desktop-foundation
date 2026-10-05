pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "../theme"
import "../components"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    required property var lifecycle
    property var targetScreen: null
    property int selected: 0
    property bool busy: false
    property string error: ""
    readonly property var actions: [
        {
            id: "suspend",
            label: "Suspend",
            symbol: "☾",
            detail: "Put this computer to sleep"
        },
        {
            id: "logout",
            label: "Log out",
            symbol: "↗",
            detail: "Return to the login screen"
        },
        {
            id: "reboot",
            label: "Reboot",
            symbol: "↻",
            detail: "Restart this computer"
        },
        {
            id: "poweroff",
            label: "Power off",
            symbol: "⏻",
            detail: "Shut down this computer"
        }
    ]
    screen: targetScreen
    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-power"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"
    function navigate(delta: int): void {
        if (!busy)
            selected = Math.max(0, Math.min(actions.length - 1, selected + delta));
    }
    function dismiss(): void {
        if (!busy)
            lifecycle.powerEnabled = false;
    }
    function choose(index: int): void {
        if (busy)
            return;
        selected = index;
        error = "";
        execute();
    }
    function execute(): void {
        if (busy)
            return;
        busy = true;
        actionProcess.command = [Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/power-action", actions[selected].id];
        actionProcess.running = true;
    }
    Process {
        id: actionProcess
        stderr: StdioCollector {
            onStreamFinished: root.error = text.trim()
        }
        // qmllint disable signal-handler-parameters
        // Installed qmltypes omit QProcess::ExitStatus; only exitCode is used.
        onExited: exitCode => {
            root.busy = false;
            if (exitCode === 0)
                root.dismiss();
            else if (!root.error)
                root.error = "The session action could not be completed.";
        }
        // qmllint enable signal-handler-parameters
    }
    Rectangle {
        anchors.fill: parent
        color: Theme.colors.scrim
        MouseArea {
            anchors.fill: parent
            onClicked: root.dismiss()
        }
    }
    Rectangle {
        id: card
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        anchors.centerIn: parent
        width: Math.min(480, root.width - 48)
        height: 396
        radius: Theme.radii.surface
        color: Theme.colors.background
        border.width: 1
        border.color: Theme.colors.border
        focus: true
        Component.onCompleted: forceActiveFocus()
        MouseArea {
            anchors.fill: parent
        }
        Keys.onPressed: event => {
            const activate = event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space || (event.key >= Qt.Key_1 && event.key <= Qt.Key_4);
            if (root.busy || (activate && event.isAutoRepeat)) {
                event.accepted = true;
                return;
            }
            if (event.key === Qt.Key_Escape) {
                root.dismiss();
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                root.choose(root.selected);
            } else if (event.key === Qt.Key_Down || event.key === Qt.Key_Right || (event.key === Qt.Key_Tab && !(event.modifiers & Qt.ShiftModifier))) {
                root.selected = (root.selected + 1) % root.actions.length;
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Left || event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))) {
                root.selected = (root.selected + root.actions.length - 1) % root.actions.length;
            } else if (event.key === Qt.Key_Home || event.key === Qt.Key_PageUp) {
                root.selected = 0;
            } else if (event.key === Qt.Key_End || event.key === Qt.Key_PageDown) {
                root.selected = root.actions.length - 1;
            } else if (event.key >= Qt.Key_1 && event.key <= Qt.Key_4) {
                root.choose(event.key - Qt.Key_1);
            } else {
                return;
            }
            event.accepted = true;
        }
        Column {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 8
            Text {
                text: "Session"
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.heading
            }

            Item {
                width: 1
                height: 8
            }
            Column {
                width: parent.width
                spacing: 6
                visible: true
                Repeater {
                    model: root.actions
                    Rectangle {
                        id: row
                        required property var modelData
                        required property int index
                        width: parent.width
                        height: 52
                        radius: Theme.radii.small
                        color: root.selected === index ? Theme.colors.selected : (mouse.containsMouse ? Theme.colors.hover : "transparent")
                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 14
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.modelData.symbol
                            color: Theme.colors.accent
                            font.pixelSize: 22
                        }
                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 52
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.modelData.label
                            color: Theme.colors.foreground
                            font.family: Theme.typography.family
                            font.pixelSize: Theme.typography.body
                        }
                        Text {
                            anchors.right: parent.right
                            anchors.rightMargin: 14
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.index + 1
                            color: Theme.colors.subtle
                            font.family: Theme.typography.mono
                            font.pixelSize: Theme.typography.small
                        }
                        MouseArea {
                            id: mouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.choose(row.index)
                        }
                    }
                }
            }
        }
        Text {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 24
            anchors.bottomMargin: 42
            text: root.error
            color: Theme.colors.error
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
            wrapMode: Text.Wrap
        }
        Text {
            anchors.bottom: parent.bottom
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottomMargin: 20
            text: "↑ ↓ / Tab navigate · Enter / Space execute · Esc close"
            color: Theme.colors.subtle
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
        }
    }
}
