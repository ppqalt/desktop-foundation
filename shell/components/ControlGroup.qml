import QtQuick
import QtQuick.Layouts
import "../theme"

// M3 filled card: medium corners, surfaceContainerLow, 16dp content inset.
Rectangle {
    id: root
    default property alias content: layout.data
    implicitHeight: layout.implicitHeight + 32
    implicitWidth: 240
    radius: Material.shape.medium
    color: Material.colors.surface_container_low ?? "#1d1b20"
    ColumnLayout {
        id: layout
        anchors.fill: parent
        anchors.margins: 16
        spacing: 8
    }
}
