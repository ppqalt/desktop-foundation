pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "../components"
import "../theme"

// Four tiny, input-transparent surfaces per output. Top is naturally below
// native fullscreen content and the secure session lock; no polling is needed.
Scope {
    Variants {
        model: Quickshell.screens
        Scope {
            id: output
            required property var modelData
            Variants {
                model: [0, 1, 2, 3]
                PanelWindow {
                    required property int modelData
                    screen: output.modelData
                    color: "transparent"
                    implicitWidth: Material.shape.extraLarge
                    implicitHeight: Material.shape.extraLarge
                    anchors.top: modelData < 2
                    anchors.bottom: modelData >= 2
                    anchors.left: modelData === 0 || modelData === 3
                    anchors.right: modelData === 1 || modelData === 2
                    exclusionMode: ExclusionMode.Ignore
                    mask: Region {}
                    WlrLayershell.namespace: "desktop-foundation-screen-corner"
                    WlrLayershell.layer: WlrLayer.Top
                    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
                    ScreenCornerMask {
                        anchors.fill: parent
                        corner: modelData
                    }
                }
            }
        }
    }
}
