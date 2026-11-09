import QtQuick
import QtQuick.Controls
import "../theme"

// Range selection shares the slider's geometry, roles and native keyboard input.
RangeSlider {
    id: root
    from: 0
    to: 100
    stepSize: 1
    padding: Material.control.sliderPadding
    implicitHeight: Material.control.target
    property bool interactionDirty: false
    property real minimumSpan: 0
    property string startLabel: "Selection start"
    property string endLabel: "Selection end"
    signal previewRequested(real start, real end)
    signal commitRequested(real start, real end)
    function requestPreview(): void {
        if (second.value - first.value < minimumSpan) {
            if (first.pressed || first.handle.activeFocus)
                first.value = Math.max(from, second.value - minimumSpan);
            else
                second.value = Math.min(to, first.value + minimumSpan);
        }
        previewRequested(first.value, second.value);
        if (first.pressed || second.pressed)
            interactionDirty = true;
        else
            commitRequested(first.value, second.value);
    }
    function released(): void {
        if (!first.pressed && !second.pressed && interactionDirty) {
            interactionDirty = false;
            commitRequested(first.value, second.value);
        }
    }
    first.onMoved: root.requestPreview()
    second.onMoved: root.requestPreview()
    first.onPressedChanged: root.released()
    second.onPressedChanged: root.released()
    background: Rectangle {
        x: root.leftPadding
        y: (root.height - height) / 2
        width: root.availableWidth
        height: Material.control.sliderTrackHeight
        radius: Material.shape.small
        color: Material.colors.secondary_container
        opacity: root.enabled ? 1 : Material.state.disabledContainer
        Rectangle {
            x: Math.min(root.first.visualPosition, root.second.visualPosition) * parent.width
            width: Math.max(0, Math.abs(root.second.visualPosition - root.first.visualPosition) * parent.width)
            height: parent.height
            radius: parent.radius
            color: Material.colors.primary
        }
    }
    first.handle: Rectangle {
        x: root.leftPadding + root.first.visualPosition * (root.availableWidth - width)
        y: (root.height - height) / 2
        width: 4
        height: Material.control.sliderHandleHeight
        radius: 2
        color: Material.colors.primary
        opacity: root.enabled ? 1 : Material.state.disabledContent
        Accessible.name: root.startLabel
        FocusIndicator {
            anchors.fill: parent
            visible: root.first.handle.activeFocus
            cornerRadius: 2
        }
    }
    second.handle: Rectangle {
        x: root.leftPadding + root.second.visualPosition * (root.availableWidth - width)
        y: (root.height - height) / 2
        width: 4
        height: Material.control.sliderHandleHeight
        radius: 2
        color: Material.colors.primary
        opacity: root.enabled ? 1 : Material.state.disabledContent
        Accessible.name: root.endLabel
        FocusIndicator {
            anchors.fill: parent
            visible: root.second.handle.activeFocus
            cornerRadius: 2
        }
    }
}
