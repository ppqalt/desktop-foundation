import QtQuick
import QtQuick.Layouts
import "../theme"

// Tonal card with a shared quiet outline and 16px content inset.
Rectangle {
    id: root
    default property alias content: layout.data
    implicitHeight: layout.implicitHeight + 32
    implicitWidth: 240
    radius: Material.shape.medium
    color: Material.colors.surface_container_low ?? "#1d1b20"
    border.width: 1
    border.color: Qt.alpha(Material.colors.primary, 0.65)
    Behavior on color {
        SurfaceColorAnimation {}
    }
    Behavior on border.color {
        SurfaceColorAnimation {}
    }
    SurfaceOutline {
        cornerRadius: parent.radius
    }
    ColumnLayout {
        id: layout
        anchors.fill: parent
        anchors.margins: 16
        spacing: 8
    }
}
