import QtQuick
import "../theme"

TextInput {
    id: root
    property string placeholderText: "Search…"
    opacity: enabled ? 1 : Material.state.disabledContent
    color: Material.colors.on_surface
    selectionColor: Material.colors.secondary_container
    selectedTextColor: Material.colors.on_secondary_container
    font.family: Theme.typography.family
    font.pixelSize: Material.type.bodyLarge
    clip: true
    selectByMouse: true
    activeFocusOnPress: true
    activeFocusOnTab: true
    readonly property real preferredFieldHeight: Math.max(Material.control.target, Material.type.bodyLargeLine + 16)
    verticalAlignment: TextInput.AlignVCenter
    TapHandler {
        id: press
        target: null
        gesturePolicy: TapHandler.DragThreshold
    }
    StateLayer {
        color: Material.colors.primary
        pressed: press.pressed
    }
    Text {
        anchors.fill: parent
        verticalAlignment: Text.AlignVCenter
        visible: !root.text
        text: root.placeholderText
        color: Material.colors.on_surface_variant
        font: root.font
    }
}
