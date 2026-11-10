import QtQuick
import "../theme"

// One centered-popup presentation; geometry/content remains declarative.
SurfaceCard {
    id: root
    property bool presented: false
    property bool dismissing: false
    // Also work as a Qt Quick Controls Popup background, where the caller
    // supplies no explicit card geometry. Desktop surfaces override these.
    width: parent?.width ?? 0
    height: parent?.height ?? 0
    anchors.centerIn: parent
    opacity: presented && !dismissing ? 1 : 0
    scale: presented && !dismissing ? 1 : 0.97
    Behavior on opacity {
        SurfaceAnimation {
            duration: root.dismissing ? Theme.timing.exit : Theme.timing.normal
        }
    }
    Behavior on scale {
        SurfaceAnimation {
            duration: root.dismissing ? Theme.timing.exit : Theme.timing.normal
        }
    }
}
