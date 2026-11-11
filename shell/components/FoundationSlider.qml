import QtQuick
import QtQuick.Controls
import "../theme"
import "SliderInput.js" as SliderInput

// M3 Expressive small slider: 24dp track, 44x4dp handle, 8dp corners,
// 6dp handle/track gap, 48dp minimum interactive height. No value animation.
Slider {
    id: root
    PointerFocus {}
    from: 0
    to: 100
    stepSize: 1
    padding: Material.control.sliderPadding
    activeFocusOnTab: true
    // A wheel over any slider scrolls its containing view, never changes audio.
    wheelEnabled: false
    // Preview during a drag; commit once on release. Keyboard steps commit
    // immediately. External value updates never emit either signal.
    signal previewRequested(real nextValue)
    signal commitRequested(real nextValue)
    property bool interactionDirty: false
    onMoved: {
        previewRequested(value);
        if (pressed)
            interactionDirty = true;
        else
            commitRequested(value);
    }
    onPressedChanged: {
        if (pressed)
            interactionDirty = false;
        else if (interactionDirty) {
            interactionDirty = false;
            commitRequested(value);
        }
    }
    implicitHeight: Material.control.target
    Keys.onPressed: event => {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) {
            event.accepted = !!Material.direction(event.key) || [Qt.Key_Home, Qt.Key_End].includes(event.key);
            return;
        }
        const previous = value;
        if (SliderInput.adjust(event, root, from, to, mirrored)) {
            root.focusReason = Qt.TabFocusReason;
            root.forceActiveFocus(Qt.TabFocusReason);
            if (value !== previous)
                moved();
            event.accepted = true;
            return;
        }
        const direction = Material.direction(event.key);
        if (direction && direction[1]) {
            Material.moveFocus(root, direction[0], direction[1]);
            event.accepted = true;
        }
    }

    readonly property real handleCenter: leftPadding + Material.control.sliderHandleWidth / 2 + visualPosition * (availableWidth - Material.control.sliderHandleWidth)
    readonly property real trackClearance: Material.control.sliderGap + Material.control.sliderHandleWidth / 2
    readonly property color activeColor: enabled ? Material.colors.primary : Material.colors.on_surface
    background: Item {
        x: root.leftPadding
        y: (root.height - height) / 2
        width: root.availableWidth
        height: Material.control.sliderTrackHeight
        opacity: root.enabled ? 1 : Material.state.disabledContainer
        Rectangle {
            width: Math.max(0, root.handleCenter - root.leftPadding - root.trackClearance)
            height: parent.height
            radius: Material.control.sliderTrackRadius
            color: root.activeColor
        }
        Rectangle {
            x: root.handleCenter - root.leftPadding + root.trackClearance
            width: Math.max(0, parent.width - x)
            height: parent.height
            radius: Material.control.sliderTrackRadius
            color: Material.colors.secondary_container
            Rectangle {
                anchors.right: parent.right
                anchors.rightMargin: 4
                anchors.verticalCenter: parent.verticalCenter
                width: 4
                height: 4
                radius: 2
                visible: parent.width > 12
                color: Material.colors.on_secondary_container
            }
        }
        StateLayer {
            color: Material.colors.on_surface
            pressed: root.pressed
            cornerRadius: Material.control.sliderTrackRadius
        }
    }
    handle: Rectangle {
        x: root.handleCenter - width / 2
        y: (root.height - height) / 2
        width: Math.max(0, handleShape.value)
        SpringValue {
            id: handleShape
            targetValue: root.pressed ? Material.control.sliderPressedHandleWidth : Material.control.sliderHandleWidth
        }
        height: Material.control.sliderHandleHeight
        radius: Material.control.sliderHandleRadius
        color: root.activeColor
        opacity: root.enabled ? 1 : Material.state.disabledContent
        FocusIndicator {
            cornerRadius: parent.radius
            visible: root.visualFocus
        }
    }
}
