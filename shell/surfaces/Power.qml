pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "../theme"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    required property var lifecycle
    property var targetScreen: null
    property int selected: 0
    property bool confirming: false
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
    function dismiss(): void {
        if (!busy)
            lifecycle.powerEnabled = false;
    }
    function choose(index: int): void {
        if (busy)
            return;
        selected = index;
        confirming = true;
        error = "";
    }
    function confirm(): void {
        if (!confirming || busy)
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
        onExited: (exitCode, exitStatus) => {
            root.busy = false;
            if (exitCode === 0)
                root.dismiss();
            else if (!root.error)
                root.error = "The session action could not be completed.";
        }
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
            if (event.isAutoRepeat || root.busy) {
                event.accepted = true;
                return;
            }
            if (event.key === Qt.Key_Escape) {
                if (root.confirming)
                    root.confirming = false;
                else
                    root.dismiss();
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                if (root.confirming)
                    root.confirm();
                else
                    root.choose(root.selected);
            } else if (!root.confirming && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
                root.selected = (root.selected + (event.key === Qt.Key_Down ? 1 : 3)) % 4;
            } else if (!root.confirming && event.key >= Qt.Key_1 && event.key <= Qt.Key_4) {
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
                text: root.confirming ? root.actions[root.selected].label : "Session"
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.heading
            }
            Text {
                visible: root.confirming
                text: "Confirm this action to continue."
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: Theme.typography.body
            }
            Item {
                width: 1
                height: 8
            }
            Column {
                width: parent.width
                spacing: 6
                visible: !root.confirming
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
            Column {
                width: parent.width
                spacing: 16
                visible: root.confirming
                Text {
                    width: parent.width
                    text: root.actions[root.selected].detail + "."
                    color: Theme.colors.foreground
                    font.family: Theme.typography.family
                    font.pixelSize: Theme.typography.body
                    wrapMode: Text.Wrap
                }
                Text {
                    width: parent.width
                    text: root.selected === 1 || root.selected === 2 || root.selected === 3 ? "Save your work before continuing." : "Your session will resume when you wake it."
                    color: Theme.colors.muted
                    font.family: Theme.typography.family
                    font.pixelSize: Theme.typography.body
                    wrapMode: Text.Wrap
                }
                Rectangle {
                    width: parent.width
                    height: 48
                    radius: Theme.radii.small
                    color: Theme.colors.selected
                    Text {
                        anchors.centerIn: parent
                        text: root.busy ? "Please wait…" : "Confirm " + root.actions[root.selected].label.toLowerCase()
                        color: Theme.colors.foreground
                        font.family: Theme.typography.family
                        font.pixelSize: Theme.typography.body
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: root.confirm()
                    }
                }
                Text {
                    width: parent.width
                    text: root.error
                    color: Theme.colors.error
                    font.family: Theme.typography.family
                    font.pixelSize: Theme.typography.small
                    wrapMode: Text.Wrap
                }
            }
        }
        Text {
            anchors.bottom: parent.bottom
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottomMargin: 20
            text: root.confirming ? "Enter to confirm · Esc to go back" : "↑ ↓ navigate · Enter select · Esc close"
            color: Theme.colors.subtle
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
        }
    }
}
