pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "../theme"
import "../components"

// qmllint disable uncreatable-type
PopupWindow {
    id: root
    required property var lifecycle
    property int selected: 0
    property bool busy: false
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
    surfaceNamespace: "desktop-foundation-power"
    function navigate(delta: int): void {
        if (!busy && !closing)
            selected = Material.listIndex(selected + delta, actions.length);
    }
    function present(): void {
        if (busy)
            return;
        beginPresentation();
    }
    function dismiss(): void {
        if (busy || closing)
            return;
        beginDismissal();
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
    ControlRequest {
        id: actionProcess
        expectJson: false
        onFinished: code => {
            root.busy = false;
            if (code === 0)
                root.dismiss();
            else
                root.error = failure || "The session action could not be completed.";
        }
    }
    motionItem: card
    initialFocusItem: card
    closeBlocked: root.busy
    canDispose: !root.busy
    onDisposalReady: root.lifecycle.powerEnabled = false
    onCloseRequested: root.dismiss()
    PopupCard {
        id: card
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        width: Material.popupWidth(560, root.width)
        height: Material.popupHeight(28 + root.actions.length * 76 + (root.error ? 38 : 0) + (root.busy ? 48 : 0), root.height)
        presented: root.entered
        dismissing: root.closing
        focus: true
        MouseArea {
            anchors.fill: parent
        }
        Keys.priority: Keys.BeforeItem
        Keys.onPressed: event => {
            const activate = Material.isActivation(event) || (event.key >= Qt.Key_1 && event.key <= Qt.Key_4);
            if (root.busy || root.closing || ((activate || event.key === Qt.Key_Escape) && event.isAutoRepeat)) {
                event.accepted = true;
                return;
            }
            if (event.key === Qt.Key_Escape) {
                root.dismiss();
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                root.choose(root.selected);
            } else if (event.key === Qt.Key_Down || event.key === Qt.Key_S || event.key === Qt.Key_D || event.key === Qt.Key_Right || (event.key === Qt.Key_Tab && !(event.modifiers & Qt.ShiftModifier))) {
                root.navigate(1);
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_W || event.key === Qt.Key_A || event.key === Qt.Key_Left || event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))) {
                root.navigate(-1);
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
            height: Math.max(1, card.height - 32 - (root.error ? 38 : 0) - (root.busy ? 48 : 0))
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
        WidgetStatus {
            x: 28
            y: list.y + list.height + 8
            width: card.width - 56
            message: root.error
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
