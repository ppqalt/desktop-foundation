import QtQuick
import "../theme"

Rectangle {
    property real cornerRadius: Material.shape.medium
    anchors.fill: parent
    anchors.margins: -3
    radius: cornerRadius + 3
    color: "transparent"
    border.width: 3
    border.color: Material.colors.primary
    // Caller controls visibility; this decoration never owns input or focus.
}
