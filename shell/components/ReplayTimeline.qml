pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import "../theme"

// The timeline selects footage inside an immutable source, never buffer length.
ColumnLayout {
    id: root
    property var thumbnails: []
    property real duration: 0
    property real startTime: 0
    property real endTime: duration
    property real playhead: 0
    property real frameStep: 1 / 60
    signal selectionRequested(real start, real end)
    signal scrubRequested(real seconds)
    spacing: 0
    Item {
        Layout.fillWidth: true
        implicitHeight: 68
        clip: true
        Row {
            anchors.fill: parent
            Repeater {
                model: root.thumbnails
                Image {
                    required property var modelData
                    width: root.thumbnails.length ? parent.width / root.thumbnails.length : 0
                    height: parent.height
                    source: modelData.url
                    asynchronous: true
                    fillMode: Image.PreserveAspectCrop
                    sourceSize.width: 240
                    sourceSize.height: 136
                }
            }
        }
        Rectangle {
            anchors.fill: parent
            color: "transparent"
            border.width: 1
            border.color: Material.colors.outline_variant
        }
        Rectangle {
            width: root.duration > 0 ? parent.width * root.startTime / root.duration : 0
            height: parent.height
            color: Material.colors.scrim ?? Material.colors.surface
            opacity: 0.65
        }
        Rectangle {
            x: root.duration > 0 ? parent.width * root.endTime / root.duration : parent.width
            width: Math.max(0, parent.width - x)
            height: parent.height
            color: Material.colors.scrim ?? Material.colors.surface
            opacity: 0.65
        }
        Rectangle {
            x: root.duration > 0 ? parent.width * root.startTime / root.duration : 0
            width: root.duration > 0 ? parent.width * (root.endTime - root.startTime) / root.duration : 0
            height: parent.height
            color: "transparent"
            border.width: 2
            border.color: Material.colors.primary
        }
        Rectangle {
            x: root.duration > 0 ? parent.width * root.playhead / root.duration : 0
            width: 2
            height: parent.height
            color: Material.colors.on_surface
        }
        MouseArea {
            anchors.fill: parent
            enabled: root.duration > 0
            onPressed: mouse => root.scrubRequested(Math.max(0, Math.min(root.duration, mouse.x / width * root.duration)))
            onPositionChanged: mouse => {
                if (pressed)
                    root.scrubRequested(Math.max(0, Math.min(root.duration, mouse.x / width * root.duration)));
            }
        }
    }
    FoundationRangeSlider {
        id: selection
        objectName: "replaySelection"
        Layout.fillWidth: true
        enabled: root.duration > 0
        from: 0
        to: Math.max(0.05, root.duration)
        stepSize: root.frameStep
        minimumSpan: 0.05
        first.value: root.startTime
        second.value: root.endTime
        startLabel: "Clip start"
        endLabel: "Clip end"
        onPreviewRequested: (start, end) => {
            root.selectionRequested(start, end);
            first.value = Qt.binding(() => root.startTime);
            second.value = Qt.binding(() => root.endTime);
        }
    }
    FoundationSlider {
        objectName: "replayPlayhead"
        Layout.fillWidth: true
        Accessible.name: "Scrub replay"
        enabled: root.duration > 0
        from: 0
        to: Math.max(0.05, root.duration)
        stepSize: root.frameStep
        value: root.playhead
        onPreviewRequested: seconds => root.scrubRequested(seconds)
    }
}
