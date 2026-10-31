pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "../theme"

// Centered interactive-window foundation; content/backend operations stay outside.
// qmllint disable uncreatable-type
PanelWindow {
    id: root
    property var targetScreen: null
    property bool sharedBackdrop: false
    property bool handoff: false
    property bool superseded: false
    property string surfaceNamespace: ""
    property Item motionItem: null
    property Item initialFocusItem: null
    property alias entered: presentation.entered
    property alias closing: presentation.closing
    property alias closeBlocked: presentation.closeBlocked
    property alias canDispose: presentation.canDispose
    readonly property string phase: presentation.phase
    signal closeRequested
    signal disposalReady
    screen: targetScreen
    visible: !superseded
    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: surfaceNamespace
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: closing || superseded ? WlrKeyboardFocus.None : WlrKeyboardFocus.Exclusive
    color: "transparent"
    function restoreInitialFocus(): void {
        if (initialFocusItem && !closing && !superseded)
            Material.restoreFocus(initialFocusItem, Qt.PopupFocusReason);
    }
    function beginPresentation(): void {
        presentation.present();
        restoreInitialFocus();
    }
    function beginDismissal(): bool {
        if (!presentation.dismiss())
            return false;
        if (initialFocusItem)
            initialFocusItem.focus = false;
        return true;
    }
    Component.onCompleted: restoreInitialFocus()
    PopupLifecycle {
        id: presentation
        targetItem: root.motionItem
        onFinished: root.disposalReady()
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
            onClicked: root.closeRequested()
        }
    }
}
