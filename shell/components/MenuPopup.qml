import QtQuick
import QtQuick.Controls
import "../theme"

// Keep the menu mapped through exit; Qt owns interruption/reopen and focus scope.
Popup {
    id: root
    property bool dismissing: false
    opacity: 0
    onAboutToShow: dismissing = false
    onAboutToHide: dismissing = true
    enter: Transition {
        SurfaceAnimation {
            property: "opacity"
            to: 1
            duration: Theme.timing.fast
        }
    }
    exit: Transition {
        SurfaceAnimation {
            property: "opacity"
            to: 0
            duration: Theme.timing.exit
        }
    }
    background: SurfaceCard {
        radius: Material.shape.small
        color: Material.colors.surface_container
    }
}
