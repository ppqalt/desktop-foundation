pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Shapes
import "../theme"

// A passive physical-screen cutout, also used by the component gallery.
// Black represents the area beyond the rounded display, not a palette surface.
Item {
    id: root
    property int corner: 0 // clockwise: top-left, top-right, bottom-right, bottom-left
    property real cornerRadius: Material.shape.extraLarge
    implicitWidth: Math.max(0, cornerRadius)
    implicitHeight: Math.max(0, cornerRadius)
    Accessible.ignored: true
    Shape {
        anchors.fill: parent
        rotation: root.corner * 90
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeWidth: -1
            fillColor: "black"
            startX: 0
            startY: 0
            PathLine { x: root.width; y: 0 }
            // Standard cubic approximation of a circular quarter arc.
            PathCubic {
                x: 0
                y: root.height
                control1X: root.width * 0.4477152501692064
                control1Y: 0
                control2X: 0
                control2Y: root.height * 0.4477152501692064
            }
            PathLine { x: 0; y: 0 }
        }
    }
}
