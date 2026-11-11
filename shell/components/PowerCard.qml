import QtQuick
import "../theme"

// Production and gallery share geometry, circle morph, icons and confirmation.
PopupCard {
    id: root
    property real availableWidth: parent?.width ?? 560
    property real availableHeight: parent?.height ?? 640
    property bool busy: false
    property string error: ""
    property alias selected: choices.selected
    property alias armed: choices.armed
    readonly property var actions: choices.actions
    signal confirmed(string action)
    function choose(index: int): void {
        choices.activate(index);
    }
    function cancelConfirmation(): void {
        choices.cancel();
    }
    function navigate(delta: int): void {
        choices.navigate(delta);
    }
    width: Material.popupWidth(armed >= 0 ? 232 : 560, availableWidth)
    height: Material.popupHeight(32 + choices.implicitHeight + (error ? 44 : 0), availableHeight)
    radius: armed >= 0 ? Math.min(width, height) / 2 : Material.shape.extraLarge
    focus: true
    Behavior on width {
        SurfaceAnimation {}
    }
    Behavior on height {
        SurfaceAnimation {}
    }
    Behavior on radius {
        SurfaceAnimation {}
    }
    MouseArea {
        anchors.fill: parent
    }
    PowerActions {
        id: choices
        x: 16
        y: 16
        width: parent.width - 32
        height: implicitHeight
        enabled: root.enabled && !root.busy && !root.dismissing
        onConfirmed: action => root.confirmed(action)
    }
    WidgetStatus {
        x: 28
        y: choices.y + choices.height + 8
        width: parent.width - 56
        message: root.error
    }
}
