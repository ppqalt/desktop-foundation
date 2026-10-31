import QtQuick
import "../theme"

// A cheap tonal halo, not a blurred shadow/offscreen effect.
Item {
    property real cornerRadius: Material.shape.extraLarge
    anchors.fill: parent
    Rectangle {
        anchors.fill: parent
        anchors.margins: -1
        radius: parent.cornerRadius + 1
        color: "transparent"
        border.width: 3
        border.color: Qt.alpha(Material.colors.primary, 0.10)
    }
}
