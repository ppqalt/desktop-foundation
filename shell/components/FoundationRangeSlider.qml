import QtQuick
import QtQuick.Controls
import "../theme"
import "SliderInput.js" as SliderInput

// Range selection shares the slider's geometry, roles and native keyboard input.
RangeSlider {
    id: root
    from: 0
    to: 100
    stepSize: 1
    wheelEnabled: false
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
    function handleKey(event, node, handle): void {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) {
            event.accepted = !!Material.direction(event.key) || [Qt.Key_Home, Qt.Key_End].includes(event.key);
            return;
        }
        const span = Math.max(0, Math.min(to - from, minimumSpan));
        const minimum = node === root.first ? from : Math.min(to, first.value + span);
        const maximum = node === root.first ? Math.max(from, second.value - span) : to;
        const previous = node.value;
        if (SliderInput.adjust(event, node, minimum, maximum, mirrored)) {
            Material.restoreFocus(handle, Qt.TabFocusReason);
            if (node.value !== previous)
                node.moved();
            event.accepted = true;
            return;
        }
        const direction = Material.direction(event.key);
        if (direction && direction[1]) {
            Material.moveFocus(handle, direction[0], direction[1]);
            event.accepted = true;
        }
    }

    first.onMoved: root.requestPreview()
    second.onMoved: root.requestPreview()
    first.onPressedChanged: {
        if (first.pressed)
            first.handle.focusReason = Qt.MouseFocusReason;
        root.released();
    }
    second.onPressedChanged: {
        if (second.pressed)
            second.handle.focusReason = Qt.MouseFocusReason;
        root.released();
    }
    readonly property real firstCenter: leftPadding + Material.control.sliderHandleWidth / 2 + first.visualPosition * (availableWidth - Material.control.sliderHandleWidth)
    readonly property real secondCenter: leftPadding + Material.control.sliderHandleWidth / 2 + second.visualPosition * (availableWidth - Material.control.sliderHandleWidth)
    readonly property real trackClearance: Material.control.sliderGap + Material.control.sliderHandleWidth / 2
    readonly property color activeColor: enabled ? Material.colors.primary : Material.colors.on_surface
    background: Item {
        x: root.leftPadding
        y: (root.height - height) / 2
        width: root.availableWidth
        height: Material.control.sliderTrackHeight
        opacity: root.enabled ? 1 : Material.state.disabledContainer
        readonly property real low: Math.min(root.firstCenter, root.secondCenter) - root.leftPadding
        readonly property real high: Math.max(root.firstCenter, root.secondCenter) - root.leftPadding
        Rectangle {
            width: Math.max(0, parent.low - root.trackClearance)
            height: parent.height
            radius: Material.control.sliderTrackRadius
            color: Material.colors.secondary_container
        }
        Rectangle {
            x: parent.low + root.trackClearance
            width: Math.max(0, parent.high - root.trackClearance - x)
            height: parent.height
            radius: Material.control.sliderTrackRadius
            color: root.activeColor
        }
        Rectangle {
            x: parent.high + root.trackClearance
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
            pressed: root.first.pressed || root.second.pressed
            cornerRadius: Material.control.sliderTrackRadius
        }
    }
    first.handle: Rectangle {
        property int focusReason: Qt.OtherFocusReason
        readonly property bool visualFocus: activeFocus && (focusReason === Qt.TabFocusReason || focusReason === Qt.BacktabFocusReason)
        x: root.firstCenter - width / 2
        y: (root.height - height) / 2
        width: Math.max(0, handleShape.value)
        SpringValue {
            id: handleShape
            targetValue: root.first.pressed ? Material.control.sliderPressedHandleWidth : Material.control.sliderHandleWidth
        }
        height: Material.control.sliderHandleHeight
        radius: Material.control.sliderHandleRadius
        color: root.activeColor
        opacity: root.enabled ? 1 : Material.state.disabledContent
        activeFocusOnTab: true
        Keys.onPressed: event => root.handleKey(event, root.first, root.first.handle)
        Keys.onReleased: event => {
            if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                focusReason = event.modifiers & Qt.ShiftModifier ? Qt.BacktabFocusReason : Qt.TabFocusReason;
        }
        onActiveFocusChanged: {
            if (!activeFocus)
                focusReason = Qt.OtherFocusReason;
        }
        Accessible.name: root.startLabel
        FocusIndicator {
            visible: root.first.handle.visualFocus
            cornerRadius: Material.control.sliderHandleRadius
        }
    }
    second.handle: Rectangle {
        property int focusReason: Qt.OtherFocusReason
        readonly property bool visualFocus: activeFocus && (focusReason === Qt.TabFocusReason || focusReason === Qt.BacktabFocusReason)
        x: root.secondCenter - width / 2
        y: (root.height - height) / 2
        width: Math.max(0, secondHandleShape.value)
        SpringValue {
            id: secondHandleShape
            targetValue: root.second.pressed ? Material.control.sliderPressedHandleWidth : Material.control.sliderHandleWidth
        }
        height: Material.control.sliderHandleHeight
        radius: Material.control.sliderHandleRadius
        color: root.activeColor
        opacity: root.enabled ? 1 : Material.state.disabledContent
        activeFocusOnTab: true
        Keys.onPressed: event => root.handleKey(event, root.second, root.second.handle)
        Keys.onReleased: event => {
            if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                focusReason = event.modifiers & Qt.ShiftModifier ? Qt.BacktabFocusReason : Qt.TabFocusReason;
        }
        onActiveFocusChanged: {
            if (!activeFocus)
                focusReason = Qt.OtherFocusReason;
        }
        Accessible.name: root.endLabel
        FocusIndicator {
            visible: root.second.handle.visualFocus
            cornerRadius: Material.control.sliderHandleRadius
        }
    }
}
