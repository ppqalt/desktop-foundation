import QtQuick

// A passive observer: retain native control gestures while updating focus
// reason even when a pointer presses the already keyboard-focused control.
TapHandler {
    property Item focusTarget: parent
    signal focusedByPointer
    target: null
    gesturePolicy: TapHandler.DragThreshold
    onPressedChanged: {
        if (pressed && focusTarget) {
            focusTarget.forceActiveFocus(Qt.MouseFocusReason);
            if (typeof focusTarget.focusReason !== "undefined")
                focusTarget.focusReason = Qt.MouseFocusReason;
            focusedByPointer();
        }
    }
}
