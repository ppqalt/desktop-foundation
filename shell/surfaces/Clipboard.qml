pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "../components"
import "../services"
import "../theme"

// qmllint disable uncreatable-type
// PanelWindow becomes creatable in the Quickshell runtime.
PanelWindow {
    id: root
    required property var lifecycle
    property bool closing: false
    property bool entered: false
    property var targetScreen: null
    screen: targetScreen
    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-clipboard"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"
    ClipboardHistory {
        id: history
        onCompleted: action => {
            if (action === "copy")
                root.dismiss();
        }
    }
    readonly property var results: history.entries.filter(entry => (entry.preview + " " + entry.mime).toLocaleLowerCase().includes(search.text.toLocaleLowerCase().trim()))
    property int selectedIndex: 0
    property bool confirmClear: false
    property bool clearSelected: false
    property bool clearChoice: true
    onConfirmClearChanged: {
        if (confirmClear)
            clearButton.forceActiveFocus();
        else if (!closing)
            search.forceActiveFocus();
    }
    onClearChoiceChanged: {
        if (confirmClear) {
            if (clearChoice)
                clearButton.forceActiveFocus();
            else
                cancelButton.forceActiveFocus();
        }
    }
    onResultsChanged: {
        selectedIndex = 0;
        clearSelected = false;
    }
    function present(): void {
        closing = false;
        exitAnimation.stop();
        confirmClear = false;
        clearSelected = false;
        search.forceActiveFocus();
    }
    function dismiss(): void {
        if (closing)
            return;
        closing = true;
        search.focus = false;
        exitAnimation.start();
    }
    function launchSelected(): void {
        if (closing || confirmClear || history.busy)
            return;
        if (clearSelected)
            beginClear();
        else if (results.length > 0)
            history.request("copy", results[selectedIndex].id);
    }
    function navigate(delta: int): void {
        if (confirmClear || !results.length)
            return;
        clearSelected = false;
        selectedIndex = Math.max(0, Math.min(results.length - 1, selectedIndex + delta));
        list.positionViewAtIndex(selectedIndex, ListView.Contain);
    }
    function keyboardNavigate(delta: int): void {
        if (confirmClear)
            return;
        if (clearSelected && results.length) {
            clearSelected = false;
            selectedIndex = delta > 0 ? 0 : results.length - 1;
            list.positionViewAtIndex(selectedIndex, ListView.Contain);
        } else if (history.entries.length && (!results.length || (delta < 0 && selectedIndex === 0) || (delta > 0 && selectedIndex === results.length - 1)))
            clearSelected = true;
        else
            navigate(delta);
    }
    function tabNavigate(delta: int): void {
        const count = results.length + (history.entries.length ? 1 : 0);
        if (!count)
            return;
        const current = clearSelected ? results.length : selectedIndex;
        const next = (current + delta + count) % count;
        clearSelected = next === results.length;
        if (!clearSelected) {
            selectedIndex = next;
            list.positionViewAtIndex(selectedIndex, ListView.Contain);
        }
    }
    function beginClear(): void {
        if (closing || confirmClear || history.busy || !history.entries.length)
            return;
        clearChoice = true;
        confirmClear = true;
    }
    function acceptClear(): void {
        if (!confirmClear || history.busy)
            return;
        if (clearChoice)
            history.request("clear", "");
        confirmClear = false;
        clearSelected = false;
    }
    function deleteSelected(): void {
        if (!closing && !confirmClear && !clearSelected && !history.busy && results.length)
            history.request("delete", results[selectedIndex].id);
    }
    function handleKey(event: var): void {
        const activate = event.key === Qt.Key_Return || event.key === Qt.Key_Enter;
        const remove = event.key === Qt.Key_Delete && (event.modifiers & Qt.ControlModifier);
        if (root.closing || ((activate || remove || (root.confirmClear && event.key === Qt.Key_Space)) && event.isAutoRepeat)) {
            event.accepted = true;
            return;
        }
        if (root.confirmClear) {
            if (event.key === Qt.Key_Escape)
                root.confirmClear = false;
            else if (activate || event.key === Qt.Key_Space)
                root.acceptClear();
            else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab || event.key === Qt.Key_Left || event.key === Qt.Key_Right || event.key === Qt.Key_Up || event.key === Qt.Key_Down)
                root.clearChoice = !root.clearChoice;
            else if (event.key === Qt.Key_Home)
                root.clearChoice = false;
            else if (event.key === Qt.Key_End)
                root.clearChoice = true;
            // The confirmation owns input; typing cannot edit the query,
            // remove items, copy an item or navigate the covered list.
        } else if (event.key === Qt.Key_Escape)
            root.dismiss();
        else if (remove && (event.modifiers & Qt.ShiftModifier))
            root.beginClear();
        else if (remove)
            root.deleteSelected();
        else if (event.key === Qt.Key_Down || (event.key === Qt.Key_N && (event.modifiers & Qt.ControlModifier)))
            root.keyboardNavigate(1);
        else if (event.key === Qt.Key_Up || (event.key === Qt.Key_P && (event.modifiers & Qt.ControlModifier)))
            root.keyboardNavigate(-1);
        else if (event.key === Qt.Key_PageDown || event.key === Qt.Key_PageUp)
            root.navigate((event.key === Qt.Key_PageDown ? 1 : -1) * Math.max(1, Math.floor(list.height / 66)));
        else if (activate)
            root.launchSelected();
        else if (event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier)))
            root.tabNavigate(-1);
        else if (event.key === Qt.Key_Tab)
            root.tabNavigate(1);
        else
            return;
        event.accepted = true;
    }
    function setQuery(value: string): void {
        search.text = value;
    }
    function snapshot(): var {
        return {
            inputFocused: search.activeFocus,
            confirmationFocused: cancelButton.activeFocus || clearButton.activeFocus,
            closing: root.closing,
            query: search.text,
            count: results.length,
            selected: results[selectedIndex]?.id ?? null,
            error: history.error,
            loading: history.loading,
            confirmClear: confirmClear,
            clearSelected: clearSelected,
            clearChoice: clearChoice,
            visible: !closing
        };
    }
    Component.onCompleted: {
        lifecycle.clipboardAlive = true;
        Qt.callLater(() => {
            if (!root.closing)
                root.entered = true;
        });
        search.forceActiveFocus();
    }
    Component.onDestruction: lifecycle.clipboardAlive = false
    SequentialAnimation {
        id: exitAnimation
        PauseAnimation {
            duration: Theme.timing.exit
        }
        ScriptAction {
            script: root.lifecycle.clipboardEnabled = false
        }
    }
    Rectangle {
        anchors.fill: parent
        color: Theme.colors.scrim
        opacity: root.entered && !root.closing ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.timing.normal
            }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: root.dismiss()
        }
    }
    SurfaceCard {
        id: panel
        SelectionWheel {
            enabled: !root.confirmClear
            onStepped: delta => root.navigate(delta)
        }
        width: Math.min(root.confirmClear ? 440 : Theme.dimensions.launcherWidth, root.width - 48)
        height: Math.min(root.confirmClear ? 268 : 182 + (history.loading ? 6 : Math.max(2, Math.min(6, root.results.length))) * 66, root.height - 64)
        Behavior on width {
            // Initial output geometry uses the shared fade/scale entrance.
            // Keep the existing confirmation morph once opened (or requested).
            enabled: root.confirmClear || (root.entered && panel.opacity === 1 && !root.closing)
            NumberAnimation {
                duration: Theme.timing.normal
                easing.type: Theme.easing
            }
        }
        Behavior on height {
            enabled: root.confirmClear || (root.entered && panel.opacity === 1 && !root.closing)
            NumberAnimation {
                duration: Theme.timing.normal
                easing.type: Theme.easing
            }
        }
        anchors.centerIn: parent
        opacity: root.entered && !root.closing ? 1 : 0
        scale: root.entered && !root.closing ? 1 : 0.97
        Behavior on opacity {
            NumberAnimation {
                duration: root.closing ? Theme.timing.exit : Theme.timing.normal
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: Theme.timing.normal
                easing.type: Theme.easing
            }
        }
        // Capture blank panel clicks without moving keyboard focus.
        MouseArea {
            anchors.fill: parent
        }
        Text {
            x: 28
            y: 23
            text: "CLIPBOARD"
            font.family: Theme.typography.mono
            font.pixelSize: 10
            font.letterSpacing: 2
            color: Theme.colors.muted
        }
        Text {
            anchors.right: parent.right
            anchors.rightMargin: clearAction.width + 48
            visible: !root.confirmClear
            y: 23
            text: root.results.length + (search.text ? (root.results.length === 1 ? " match" : " matches") : " available")
            font.family: Theme.typography.mono
            font.pixelSize: 10
            color: Theme.colors.subtle
        }
        Rectangle {
            id: clearAction
            anchors.right: parent.right
            anchors.rightMargin: 22
            y: 15
            width: 154
            height: 28
            visible: !root.confirmClear
            radius: Theme.radii.small
            color: root.clearSelected ? Theme.colors.selected : (clearActionMouse.containsMouse ? Theme.colors.hover : "transparent")
            border.width: root.clearSelected ? 1 : 0
            border.color: Theme.colors.selectionBorder
            opacity: history.entries.length && !history.busy ? 1 : 0.45
            Accessible.role: Accessible.Button
            Accessible.name: "Clear all clipboard history"
            Accessible.onPressAction: root.beginClear()
            Text {
                anchors.centerIn: parent
                text: "Clear all · Ctrl⇧Del"
                color: root.clearSelected ? Theme.colors.accent : Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 11
            }
            MouseArea {
                id: clearActionMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                enabled: history.entries.length > 0 && !history.busy && !root.confirmClear
                onClicked: root.beginClear()
            }
        }
        Image {
            visible: !root.confirmClear
            x: 28
            y: 62
            width: 24
            height: 24
            source: "../assets/search.svg"
        }
        SearchInput {
            id: search
            visible: !root.confirmClear
            x: 68
            y: 57
            width: panel.width - 96
            height: 38
            placeholderText: "Search clipboard…"
            readOnly: root.confirmClear
            Keys.priority: Keys.BeforeItem
            Keys.onPressed: event => root.handleKey(event)
        }
        Rectangle {
            visible: !root.confirmClear
            x: 28
            y: 110
            width: panel.width - 56
            height: 1
            color: Theme.colors.border
        }
        ListView {
            id: list
            visible: !root.confirmClear
            enabled: !root.confirmClear
            SelectionWheel {
                enabled: !root.confirmClear
                onStepped: delta => root.navigate(delta)
            }
            x: 16
            y: 126
            width: panel.width - 32
            height: panel.height - 186
            clip: true
            spacing: 4
            model: root.results
            boundsBehavior: Flickable.StopAtBounds
            currentIndex: root.selectedIndex
            delegate: ClipboardRow {
                required property var modelData
                required property int index
                width: list.width
                entry: modelData
                selected: !root.clearSelected && index === root.selectedIndex
                onRemoved: {
                    if (!history.busy)
                        history.request("delete", modelData.id);
                }
                onChosen: {
                    root.clearSelected = false;
                    root.selectedIndex = index;
                    root.launchSelected();
                }
            }
        }
        Column {
            visible: !root.confirmClear && root.results.length === 0
            anchors.centerIn: list
            spacing: 10
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: history.loading ? "Loading clipboard…" : (history.entries.length ? "No matching items" : "Your clipboard starts here")
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: 16
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: history.entries.length ? "Try another word from the copied text." : "Copy text or an image and it will appear here."
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 12
            }
        }
        FocusScope {
            id: confirmation
            visible: root.confirmClear
            x: 28
            y: 64
            width: panel.width - 56
            height: 132
            Keys.priority: Keys.BeforeItem
            Keys.onPressed: event => root.handleKey(event)
            Text {
                width: parent.width
                text: "Clear clipboard history?"
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: 19
            }
            Text {
                y: 34
                width: parent.width
                text: "This removes all saved items."
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 12
            }
            Rectangle {
                id: cancelButton
                y: 78
                width: (parent.width - 12) / 2
                height: 44
                radius: Theme.radii.medium
                color: !root.clearChoice ? Theme.colors.selected : (cancelMouse.containsMouse ? Theme.colors.hover : "transparent")
                border.width: 1
                border.color: !root.clearChoice ? Theme.colors.selectionBorder : Theme.colors.border
                enabled: !history.busy
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: "Cancel"
                Accessible.onPressAction: root.confirmClear = false
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: event => root.handleKey(event)
                Text {
                    anchors.centerIn: parent
                    text: "Cancel"
                    color: Theme.colors.foreground
                    font.family: Theme.typography.family
                    font.pixelSize: 13
                }
                MouseArea {
                    id: cancelMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.confirmClear = false
                }
            }
            Rectangle {
                id: clearButton
                anchors.right: parent.right
                y: 78
                width: (parent.width - 12) / 2
                height: 44
                radius: Theme.radii.medium
                color: root.clearChoice ? Theme.colors.selected : (clearMouse.containsMouse ? Theme.colors.hover : "transparent")
                border.width: 1
                border.color: root.clearChoice ? Theme.colors.selectionBorder : Theme.colors.border
                enabled: !history.busy
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: "Clear clipboard history"
                Accessible.onPressAction: {
                    root.clearChoice = true;
                    root.acceptClear();
                }
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: event => root.handleKey(event)
                Text {
                    anchors.centerIn: parent
                    text: "Clear history"
                    color: Theme.colors.error
                    font.family: Theme.typography.family
                    font.pixelSize: 13
                }
                MouseArea {
                    id: clearMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.clearChoice = true;
                        root.acceptClear();
                    }
                }
            }
        }
        Text {
            x: 28
            y: panel.height - 31
            width: panel.width - 220
            visible: !!history.error
            text: history.error
            color: Theme.colors.error
            font.family: Theme.typography.family
            font.pixelSize: 11
            elide: Text.ElideRight
        }
        SurfaceFooter {
            x: 28
            y: panel.height - 48
            width: panel.width - 56
            navigationKeys: root.confirmClear ? "Tab" : "↑ ↓"
            navigationLabel: root.confirmClear ? "choose" : "navigate"
            escapeLabel: root.confirmClear ? "cancel" : "close"
            hintsVisible: !history.error
            actionText: root.confirmClear ? (root.clearChoice ? "Clear history" : "Cancel") : (history.busy ? "Copying…" : (root.clearSelected ? "Clear all" : "Copy & close"))
            actionEnabled: !root.closing && !history.busy && (root.confirmClear || root.clearSelected || root.results.length > 0)
            onActivated: {
                if (root.confirmClear)
                    root.acceptClear();
                else
                    root.launchSelected();
            }
            escapeEnabled: !root.closing && !history.busy
            onEscapeRequested: {
                if (root.confirmClear)
                    root.confirmClear = false;
                else
                    root.dismiss();
            }
        }
    }
}
