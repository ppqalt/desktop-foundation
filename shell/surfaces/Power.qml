pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
import Quickshell
import "../theme"
import "../components"

// qmllint disable uncreatable-type
PopupWindow {
    id: root
    required property var lifecycle
    property alias selected: card.selected
    property alias armed: card.armed
    property bool busy: false
    property string error: ""
    readonly property var actions: card.actions
    surfaceNamespace: "desktop-foundation-power"
    function navigate(delta: int): void {
        if (!busy && !closing)
            card.navigate(delta);
    }
    function present(): void {
        if (busy)
            return;
        card.cancelConfirmation();
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
        error = "";
        card.choose(index);
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
            else {
                card.cancelConfirmation();
                root.error = failure || "The session action could not be completed.";
            }
        }
    }
    motionItem: card
    initialFocusItem: card
    closeBlocked: root.busy
    canDispose: !root.busy
    onDisposalReady: root.lifecycle.powerEnabled = false
    onCloseRequested: root.dismiss()
    PowerCard {
        id: card
        availableWidth: root.width
        availableHeight: root.height
        busy: root.busy
        error: root.error
        presented: root.entered
        dismissing: root.closing
        onConfirmed: action => root.execute()
        SelectionWheel {
            onStepped: delta => root.navigate(delta)
        }
        Keys.priority: Keys.BeforeItem
        Keys.onPressed: event => {
            const activate = Material.isActivation(event) || (event.key >= Qt.Key_1 && event.key <= Qt.Key_4);
            if (root.busy || root.closing || ((activate || event.key === Qt.Key_Escape) && event.isAutoRepeat)) {
                event.accepted = true;
                return;
            }
            if (event.key === Qt.Key_Escape) {
                if (root.armed >= 0)
                    card.cancelConfirmation();
                else
                    root.dismiss();
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                root.choose(root.selected);
            } else if (event.key === Qt.Key_Down || event.key === Qt.Key_S || event.key === Qt.Key_D || event.key === Qt.Key_Right || (event.key === Qt.Key_Tab && !(event.modifiers & Qt.ShiftModifier))) {
                root.navigate(1);
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_W || event.key === Qt.Key_A || event.key === Qt.Key_Left || event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))) {
                root.navigate(-1);
            } else if (event.key === Qt.Key_Home || event.key === Qt.Key_PageUp) {
                if (root.armed < 0)
                    root.selected = 0;
            } else if (event.key === Qt.Key_End || event.key === Qt.Key_PageDown) {
                if (root.armed < 0)
                    root.selected = root.actions.length - 1;
            } else if (event.key >= Qt.Key_1 && event.key <= Qt.Key_4) {
                root.choose(event.key - Qt.Key_1);
            } else {
                return;
            }
            event.accepted = true;
        }
    }
}
