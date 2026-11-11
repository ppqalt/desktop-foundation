import QtQuick
import QtQuick.Controls
import "../theme"

// Compact scroll indicator/drag target; the owning view controls visibility.
ScrollBar {
    id: root
    policy: ScrollBar.AsNeeded
    padding: 0
    contentItem: Rectangle {
        implicitWidth: 4
        radius: 2
        color: root.pressed ? Material.colors.primary : Material.colors.outline
        Behavior on color {
            SurfaceColorAnimation {}
        }
    }
    background: null
}
