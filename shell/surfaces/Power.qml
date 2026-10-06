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
    property bool entered: false
    property bool closing: false
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
        if (!busy && !closing)
            selected = Math.max(0, Math.min(actions.length - 1, selected + delta));
    }
    function dismiss(): void {
        if (busy || closing)
            return;
        closing = true;
        exitAnimation.start();
    }
    function choose(index: int): void {
        if (busy || closing)
            return;
        selected = index;
        error = "";
        execute();
    }
    function execute(): void {
        if (busy || closing)
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
    Component.onCompleted: Qt.callLater(() => root.entered = true)
    SequentialAnimation {
        id: exitAnimation
        PauseAnimation {
            duration: Theme.timing.exit
        }
        ScriptAction {
            script: root.lifecycle.powerEnabled = false
        }
    }
    Rectangle {
        anchors.fill: parent
        color: Theme.colors.scrim
        opacity: root.entered && !root.closing ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: root.closing ? Theme.timing.exit : Theme.timing.normal
            }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: root.dismiss()
        }
    }
    SurfaceCard {
        id: card
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        anchors.centerIn: parent
        width: Math.min(Theme.dimensions.launcherWidth, root.width - 48)
        height: Math.min(182 + root.actions.length * 66 + (root.error ? 38 : 0), root.height - 64)
        opacity: root.entered && !root.closing ? 1 : 0
        scale: root.entered && !root.closing ? 1 : 0.97
        Behavior on opacity {
            NumberAnimation {
                duration: root.closing ? Theme.timing.exit : Theme.timing.normal
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: Theme.timing.normal
                easing.type: Theme.easing
            }
        }
        focus: true
        Component.onCompleted: forceActiveFocus()
        MouseArea {
            anchors.fill: parent
        }
        Keys.priority: Keys.BeforeItem
        Keys.onPressed: event => {
            const activate = event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space || (event.key >= Qt.Key_1 && event.key <= Qt.Key_4);
            if (root.busy || root.closing || (activate && event.isAutoRepeat)) {
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
        Text {
            x: 28
            y: 23
            text: "POWER"
            color: Theme.colors.muted
            font.family: Theme.typography.mono
            font.pixelSize: 10
            font.letterSpacing: 2
        }
        Text {
            x: 28
            y: 60
            text: "Session"
            color: Theme.colors.foreground
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.heading
        }
        Rectangle {
            x: 28
            y: 110
            width: card.width - 56
            height: 1
            color: Theme.colors.border
        }
        ListView {
            id: list
            x: 16
            y: 126
            width: card.width - 32
            height: Math.max(1, card.height - 186 - (root.error ? 38 : 0))
            clip: true
            spacing: 4
            model: root.actions
            currentIndex: root.selected
            boundsBehavior: Flickable.StopAtBounds
            delegate: ApplicationRow {
                required property var modelData
                required property int index
                width: list.width
                entry: ({
                        name: modelData.label,
                        genericName: modelData.detail
                    })
                iconText: modelData.symbol
                selected: root.selected === index
                actionLabel: String(index + 1)
                interactive: !root.busy && !root.closing
                onChosen: root.choose(index)
            }
        }
        Connections {
            target: root
            function onSelectedChanged(): void {
                list.positionViewAtIndex(root.selected, ListView.Contain);
            }
        }
        Text {
            x: 28
            y: card.height - 85
            width: card.width - 56
            visible: root.error !== ""
            text: root.error
            color: Theme.colors.error
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
            wrapMode: Text.Wrap
        }
        SurfaceFooter {
            x: 28
            y: card.height - 48
            width: card.width - 56
            actionText: root.busy ? "Working…" : root.actions[root.selected].label
            actionEnabled: !root.busy && !root.closing
            onActivated: root.choose(root.selected)
            escapeEnabled: !root.busy && !root.closing
            onEscapeRequested: root.dismiss()
        }
    }
}
