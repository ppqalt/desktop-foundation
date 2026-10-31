import QtQuick
import "../theme"

Rectangle {
    radius: Material.shape.extraLarge
    color: Material.colors.surface_container_low
    border.width: 1
    border.color: Qt.alpha(Material.colors.primary, 0.65)
    SurfaceOutline {
        cornerRadius: parent.radius
    }
}
