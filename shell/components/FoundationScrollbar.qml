import QtQuick
import QtQuick.Controls
import "../theme"

// Compact scroll indicator/drag target; the owning view controls visibility.
ScrollBar {
    policy: ScrollBar.AsNeeded
    padding: 0
    contentItem: Rectangle {
        implicitWidth: 4
        radius: 2
        color: Material.colors.outline
    }
    background: null
}
