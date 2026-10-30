pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
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
    property bool sharedBackdrop: false
    property bool handoff: false
    property bool superseded: false
    visible: !superseded
    property bool closing: false
    property string error: ""
    readonly property var actions: [
        {
            id: "suspend",
            label: "Suspend",
            symbol: "dark_mode"
        },
        {
            id: "logout",
            label: "Log out",
            symbol: "logout"
        },
        {
            id: "reboot",
            label: "Reboot",
            symbol: "restart_alt"
        },
        {
            id: "poweroff",
            label: "Power off",
            symbol: "power_settings_new"
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
    WlrLayershell.keyboardFocus: root.closing || root.superseded ? WlrKeyboardFocus.None : WlrKeyboardFocus.Exclusive
    color: "transparent"
    function navigate(delta: int): void {
        if (!busy && !closing)
            selected = Math.max(0, Math.min(actions.length - 1, selected + delta));
    }
    function present(): void {
        if (busy)
            return;
        exitAnimation.stop();
        closing = false;
        entered = true;
        card.forceActiveFocus();
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
        color: root.sharedBackdrop ? "transparent" : Theme.colors.scrim
        opacity: (root.entered || root.handoff) && !root.closing ? 1 : 0
        Behavior on opacity {
            SurfaceAnimation {
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
        // Begin when this window queues its first frame for presentation.
        // Construction can precede layer-shell configure/map: callLater is early.
        Connections {
            target: card.Window.window
            enabled: !root.entered && !root.closing
            function onFrameSwapped(): void {
                root.entered = true;
            }
        }
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        anchors.centerIn: parent
        width: Math.min(560, root.width - 48)
        height: Math.min(72 + root.actions.length * 76 + (root.error ? 38 : 0), root.height - 64)
        opacity: root.entered && !root.closing ? 1 : 0
        scale: root.entered && !root.closing ? 1 : 0.97
        Behavior on opacity {
            SurfaceAnimation {
                duration: root.closing ? Theme.timing.exit : Theme.timing.normal
            }
        }
        Behavior on scale {
            SurfaceAnimation {
                duration: root.closing ? Theme.timing.exit : Theme.timing.normal
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
            } else if (event.key === Qt.Key_Down || event.key === Qt.Key_S || event.key === Qt.Key_D || event.key === Qt.Key_Right || (event.key === Qt.Key_Tab && !(event.modifiers & Qt.ShiftModifier))) {
                root.selected = (root.selected + 1) % root.actions.length;
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_W || event.key === Qt.Key_A || event.key === Qt.Key_Left || event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))) {
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
        ListView {
            id: list
            x: 16
            y: 16
            width: card.width - 32
            height: Math.max(1, card.height - 76 - (root.error ? 38 : 0))
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
                        name: modelData.label
                    })
                iconName: modelData.symbol
                selected: root.selected === index
                actionLabel: ""
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
            color: Material.colors.error
            font.family: Theme.typography.family
            font.pixelSize: Theme.typography.small
            wrapMode: Text.Wrap
        }
        SurfaceFooter {
            x: 28
            y: card.height - 48
            width: card.width - 56
            actionText: root.busy ? "Working…" : ""
            actionEnabled: !root.busy && !root.closing
            onActivated: root.choose(root.selected)
            escapeEnabled: !root.busy && !root.closing
            onEscapeRequested: root.dismiss()
        }
    }
}
