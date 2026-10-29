import QtQuick
import "../theme"

TextInput {
    id: root
    property string placeholderText: "Search…"
    color: Material.colors.on_surface
    selectionColor: Material.colors.secondary_container
    selectedTextColor: Material.colors.on_secondary_container
    font.family: Theme.typography.family
    font.pixelSize: Material.type.bodyLarge
    clip: true
    selectByMouse: true
    activeFocusOnPress: true
    verticalAlignment: TextInput.AlignVCenter
    Text {
        anchors.fill: parent
        verticalAlignment: Text.AlignVCenter
        visible: !root.text
        text: root.placeholderText
        color: Material.colors.on_surface_variant
        font: root.font
    }
}
