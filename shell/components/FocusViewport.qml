import QtQuick
import QtQuick.Window

// One focus-reveal policy for scrollable surfaces. Works in content coordinates,
// including scaled gallery compositions; it never moves focus or owns input.
QtObject {
    id: root
    required property Flickable viewport
    property real inset: 6
    property bool followFocus: true
    property bool revealing: false
    readonly property var focusWindow: viewport.Window.window
    function reveal(): void {
        const item = focusWindow?.activeFocusItem;
        if (!followFocus || !item || !item.visible || !item.enabled || viewport.height <= 0)
            return;
        let ancestor = item;
        while (ancestor && ancestor !== viewport.contentItem)
            ancestor = ancestor.parent;
        if (!ancestor)
            return; // Popups and other surfaces retain their own scroll owners.
        const start = item.mapToItem(viewport.contentItem, 0, 0).y;
        const end = item.mapToItem(viewport.contentItem, 0, item.height).y;
        const top = Math.min(start, end) - inset;
        const bottom = Math.max(start, end) + inset;
        let next = viewport.contentY;
        if (top < next || bottom - top > viewport.height)
            next = top;
        else if (bottom > next + viewport.height)
            next = bottom - viewport.height;
        revealing = true;
        viewport.contentY = Math.max(0, Math.min(Math.max(0, viewport.contentHeight - viewport.height), next));
        revealing = false;
    }
    property Connections focusChanges: Connections {
        target: root.focusWindow
        function onActiveFocusItemChanged(): void {
            root.followFocus = true;
            Qt.callLater(root.reveal);
        }
    }
    property Connections layoutChanges: Connections {
        target: root.viewport
        function onContentYChanged(): void {
            if (!root.revealing)
                root.followFocus = false;
        }
        function onHeightChanged(): void {
            Qt.callLater(root.reveal);
        }
        function onContentHeightChanged(): void {
            Qt.callLater(root.reveal);
        }
    }
}
