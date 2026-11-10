import QtQuick
import QtQuick.Controls
import "../theme"

// One activation/navigation contract for actions and backend-owned toggles.
AbstractButton {
    id: root
    signal pointerFocused
    PointerFocus {
        onFocusedByPointer: root.pointerFocused()
    }
    activeFocusOnTab: true
    autoRepeat: false
    function activateKey(event): bool {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
            return false;
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (!event.isAutoRepeat)
                root.clicked();
            event.accepted = true;
            return true;
        }
        return false;
    }
    Keys.onPressed: event => {
        if (activateKey(event))
            return;
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
            return;
        const direction = Material.direction(event.key);
        if (direction) {
            root.focusReason = Qt.TabFocusReason;
            root.forceActiveFocus(Qt.TabFocusReason);
            Material.moveFocus(root, direction[0], direction[1]);
            event.accepted = true;
        }
    }
}
