import QtQuick
import QtQuick.Controls
import "../theme"

// One activation/navigation contract for actions and backend-owned toggles.
AbstractButton {
    id: root
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
            Material.moveFocus(root, direction[0], direction[1]);
            event.accepted = true;
        }
    }
}
