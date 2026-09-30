import QtQuick
import "../theme"

TextInput {
    id: root
    property string placeholderText: "Search…"
    color: Theme.colors.foreground
    selectionColor: Theme.colors.selected
    selectedTextColor: Theme.colors.foreground
    font.family: Theme.typography.family
    font.pixelSize: Theme.typography.heading
    clip: true
    selectByMouse: true
    activeFocusOnPress: true
    verticalAlignment: TextInput.AlignVCenter
    Text {
        anchors.fill: parent
        verticalAlignment: Text.AlignVCenter
        visible: !root.text
        text: root.placeholderText
        color: Theme.colors.subtle
        font: root.font
    }
}
