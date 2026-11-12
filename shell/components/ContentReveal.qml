import QtQuick
import "../theme"

// Retain children through collapse; only the requested open state accepts input.
Item {
    id: root
    default property alias contentData: content.data
    property bool expanded: false
    readonly property real progress: reveal.value
    readonly property bool running: reveal.running
    readonly property real contentHeight: content.childrenRect.height
    implicitHeight: Math.max(0, contentHeight * progress)
    visible: expanded || running || progress > 0
    clip: true
    FocusScope {
        id: content
        width: root.width
        height: root.contentHeight
        enabled: root.expanded
        opacity: root.progress
        y: -Material.motionDistance.reveal * (1 - root.progress)
    }
    SpringValue {
        id: reveal
        motion: "fastEffects"
        targetValue: root.expanded ? 1 : 0
    }
}
