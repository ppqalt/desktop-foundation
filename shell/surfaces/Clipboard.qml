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
        onAboutToUpdate: root.captureViewport()
        onCompleted: action => {
            if (action === "copy" || action === "copy-plain")
                root.dismiss();
        }
    }
    readonly property var results: history.entries.filter(entry => [entry.label, entry.preview, entry.detail, entry.mime, entry.kind, (entry.formats || []).join(" ")].join(" ").toLocaleLowerCase().includes(search.text.toLocaleLowerCase().trim()))
    readonly property int pinnedCount: history.entries.filter(entry => entry.pinned).length
    property int selectedIndex: 0
    property string selectedIdentity: ""
    property real selectedViewportOffset: 0
    property int selectionRevision: 0
    property string resultsQuery: ""
    property bool viewportRestorePending: false
    property bool viewportAtTop: true
    property string viewportIdentity: ""
    property real viewportOffset: 0
    property real viewportContentY: 0
    property bool confirmClear: false
    property bool clearSelected: false
    // 0 = copy, 1 = pin, 2 = copy as plain text.
    property int actionFocus: 0
    // Confirmation always starts on Cancel. Clearing pins is a distinct action.
    property int clearChoice: 0
    readonly property int clearChoiceCount: pinnedCount ? 3 : 2
    onConfirmClearChanged: {
        if (confirmClear)
            focusClearChoice();
        else if (!closing)
            search.forceActiveFocus();
    }
    onClearChoiceChanged: {
        if (confirmClear)
            focusClearChoice();
    }
    onResultsChanged: reconcileSelection()
    function focusClearChoice(): void {
        if (clearChoice === 1)
            clearButton.forceActiveFocus();
        else if (clearChoice === 2)
            clearAllButton.forceActiveFocus();
        else
            cancelButton.forceActiveFocus();
    }
    function rememberSelection(): void {
        selectedIdentity = results[selectedIndex]?.id || "";
        const row = list.itemAtIndex(selectedIndex);
        if (row)
            selectedViewportOffset = row.y + (row.groupGap || 0) - list.contentY;
    }
    function captureViewport(): void {
        if (viewportRestorePending)
            return;
        rememberSelection();
        viewportRestorePending = true;
        viewportContentY = list.contentY;
        viewportAtTop = list.contentY <= list.originY + 1;
        let index = list.indexAt(1, list.contentY + 1);
        if (index < 0) {
            for (let candidate = 0; candidate < results.length; candidate++) {
                const item = list.itemAtIndex(candidate);
                if (item && item.y + item.height > list.contentY) {
                    index = candidate;
                    break;
                }
            }
        }
        const row = index >= 0 ? list.itemAtIndex(index) : null;
        viewportIdentity = results[index]?.id || "";
        viewportOffset = row ? row.y + (row.groupGap || 0) - list.contentY : 0;
    }
    function selectIndex(index: int): void {
        selectedIndex = Math.max(0, Math.min(results.length - 1, index));
        selectedIdentity = results[selectedIndex]?.id || "";
    }
    function reconcileSelection(): void {
        const revision = ++selectionRevision;
        const queryChanged = resultsQuery !== search.text;
        resultsQuery = search.text;
        const restoreViewport = viewportRestorePending && !queryChanged;
        const savedAtTop = viewportAtTop;
        const savedIdentity = viewportIdentity;
        const savedOffset = viewportOffset;
        const savedContentY = viewportContentY;
        const found = results.findIndex(entry => entry.id === selectedIdentity);
        selectIndex(queryChanged ? 0 : (found >= 0 ? found : selectedIndex));
        if (queryChanged) {
            viewportRestorePending = false;
            list.positionViewAtBeginning();
        }
        if (!results.length)
            actionFocus = 0;
        else if (actionFocus === 2 && !plainAvailable())
            actionFocus = 0;
        // Keep scrolling independent from selection. Background copies must
        // not hide pins just to hold the selected ordinary row at one pixel.
        Qt.callLater(() => {
            if (revision !== root.selectionRevision || !restoreViewport)
                return;
            list.forceLayout();
            if (savedAtTop)
                list.positionViewAtBeginning();
            else {
                const index = root.results.findIndex(entry => entry.id === savedIdentity);
                let position = savedContentY;
                if (index >= 0) {
                    list.positionViewAtIndex(index, ListView.Beginning);
                    const row = list.itemAtIndex(index);
                    if (row)
                        position = row.y + (row.groupGap || 0) - savedOffset;
                }
                list.contentY = Math.max(list.originY, Math.min(list.originY + Math.max(0, list.contentHeight - list.height), position));
            }
            root.viewportRestorePending = false;
            root.rememberSelection();
        });
    }
    function queryChanged(): void {
        selectionRevision++;
        viewportRestorePending = false;
        clearSelected = false;
        actionFocus = 0;
        selectIndex(0);
        list.positionViewAtBeginning();
        rememberSelection();
    }
    function present(): void {
        closing = false;
        exitAnimation.stop();
        confirmClear = false;
        clearSelected = false;
        actionFocus = 0;
        search.forceActiveFocus();
    }
    function dismiss(): void {
        if (closing)
            return;
        closing = true;
        search.focus = false;
        exitAnimation.start();
    }
    function plainAvailable(): bool {
        const entry = results[selectedIndex];
        return !!entry && (entry.kind === "rich" || (!entry.kind && ["text/html", "text/rtf", "application/rtf"].includes(entry.mime)));
    }
    function launchSelected(): void {
        if (closing || confirmClear || history.busy)
            return;
        if (clearSelected)
            beginClear();
        else if (results.length > 0) {
            if (actionFocus === 1)
                pinSelected();
            else if (actionFocus === 2)
                copyPlainSelected();
            else
                history.request("copy", results[selectedIndex].id);
        }
    }
    function navigate(delta: int): void {
        if (confirmClear || !results.length || (clearSelected && delta < 0))
            return;
        const from = clearSelected ? -1 : selectedIndex;
        clearSelected = false;
        actionFocus = 0;
        selectIndex(from + delta);
        list.positionViewAtIndex(selectedIndex, ListView.Contain);
        rememberSelection();
    }
    function keyboardNavigate(delta: int): void {
        if (confirmClear)
            return;
        actionFocus = 0;
        if (history.entries.length && (!results.length || (!clearSelected && delta < 0 && selectedIndex === 0)))
            clearSelected = true;
        else
            navigate(delta);
    }
    function tabNavigate(delta: int): void {
        if (!history.entries.length)
            return;
        if (!results.length) {
            clearSelected = true;
            actionFocus = 0;
            return;
        }
        if (clearSelected) {
            if (delta < 0)
                return;
            clearSelected = false;
            selectIndex(0);
            actionFocus = 0;
        } else {
            const lastAction = plainAvailable() ? 2 : 1;
            if (actionFocus + delta >= 0 && actionFocus + delta <= lastAction)
                actionFocus += delta;
            else if (delta < 0 && selectedIndex === 0) {
                clearSelected = true;
                actionFocus = 0;
            } else if (delta > 0 && selectedIndex === results.length - 1) {
                return;
            } else {
                selectIndex(selectedIndex + delta);
                actionFocus = delta > 0 ? 0 : (plainAvailable() ? 2 : 1);
            }
        }
        if (!clearSelected) {
            list.positionViewAtIndex(selectedIndex, ListView.Contain);
            rememberSelection();
        }
    }
    function beginClear(): void {
        if (closing || confirmClear || history.busy || !history.entries.length)
            return;
        clearChoice = 0;
        confirmClear = true;
    }
    function acceptClear(): void {
        if (!confirmClear || history.busy)
            return;
        if (clearChoice === 1)
            history.request("clear", "");
        else if (clearChoice === 2 && pinnedCount)
            history.request("clear-all", "");
        confirmClear = false;
        clearSelected = false;
        actionFocus = 0;
    }
    function deleteSelected(): void {
        if (!closing && !confirmClear && !clearSelected && !history.busy && results.length) {
            rememberSelection();
            history.request("delete", results[selectedIndex].id);
        }
    }
    function pinSelected(): void {
        if (!closing && !confirmClear && !clearSelected && !history.busy && results.length) {
            rememberSelection();
            history.request(results[selectedIndex].pinned ? "unpin" : "pin", results[selectedIndex].id);
        }
    }
    function copyPlainSelected(): void {
        const entry = results[selectedIndex];
        const textual = !!entry && (entry.kind === "text" || plainAvailable() || (!entry.kind && (entry.mime || "").startsWith("text/")));
        if (!closing && !confirmClear && !clearSelected && !history.busy && textual)
            history.request("copy-plain", entry.id);
    }
    function handleKey(event: var): void {
        const control = !!(event.modifiers & Qt.ControlModifier);
        const shift = !!(event.modifiers & Qt.ShiftModifier);
        const activate = event.key === Qt.Key_Return || event.key === Qt.Key_Enter;
        const remove = event.key === Qt.Key_Delete && control;
        const pin = event.key === Qt.Key_P && control && shift;
        const plain = event.key === Qt.Key_C && control && shift;
        if (root.closing || ((activate || remove || pin || plain || (root.confirmClear && event.key === Qt.Key_Space)) && event.isAutoRepeat)) {
            event.accepted = true;
            return;
        }
        if (root.confirmClear) {
            if (event.key === Qt.Key_Escape)
                root.confirmClear = false;
            else if (activate || event.key === Qt.Key_Space)
                root.acceptClear();
            else if (event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && shift) || event.key === Qt.Key_Left || event.key === Qt.Key_Up)
                root.clearChoice = (root.clearChoice + root.clearChoiceCount - 1) % root.clearChoiceCount;
            else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Right || event.key === Qt.Key_Down)
                root.clearChoice = (root.clearChoice + 1) % root.clearChoiceCount;
            else if (event.key === Qt.Key_Home)
                root.clearChoice = 0;
            else if (event.key === Qt.Key_End)
                root.clearChoice = root.clearChoiceCount - 1;
            // Modal input never reaches the hidden query or covered items.
        } else if (event.key === Qt.Key_Escape) {
            if (root.actionFocus || root.clearSelected) {
                root.actionFocus = 0;
                root.clearSelected = false;
            } else
                root.dismiss();
        } else if (pin)
            root.pinSelected();
        else if (plain)
            root.copyPlainSelected();
        else if (remove && shift)
            root.beginClear();
        else if (remove)
            root.deleteSelected();
        else if (event.key === Qt.Key_Down || (event.key === Qt.Key_N && control))
            root.keyboardNavigate(1);
        else if (event.key === Qt.Key_Up || (event.key === Qt.Key_P && control))
            root.keyboardNavigate(-1);
        else if (event.key === Qt.Key_PageDown || event.key === Qt.Key_PageUp)
            root.navigate((event.key === Qt.Key_PageDown ? 1 : -1) * Math.max(1, Math.floor(list.height / 66)));
        else if (activate)
            root.launchSelected();
        else if (event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && shift))
            root.tabNavigate(-1);
        else if (event.key === Qt.Key_Tab)
            root.tabNavigate(1);
        else if (root.actionFocus && (event.key === Qt.Key_Left || event.key === Qt.Key_Right))
            root.tabNavigate(event.key === Qt.Key_Left ? -1 : 1);
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
            confirmationFocused: cancelButton.activeFocus || clearButton.activeFocus || clearAllButton.activeFocus,
            closing: root.closing,
            query: search.text,
            count: results.length,
            selected: results[selectedIndex]?.id ?? null,
            selectedOffset: selectedViewportOffset,
            actionFocus: actionFocus,
            pinnedCount: pinnedCount,
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
        history.request("init", "");
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
        height: Math.min(root.confirmClear ? 268 : 182 + (history.loading ? 6 : Math.max(2, Math.min(6, root.results.length))) * 66 + (root.results.slice(0, 6).some(entry => entry.pinned) && root.results.slice(0, 6).some(entry => !entry.pinned) ? 12 : 0), root.height - 64)
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
            visible: !root.confirmClear && panel.width >= 540
            y: 23
            text: root.results.length + (search.text ? (root.results.length === 1 ? " match" : " matches") : (root.results.length === 1 ? " item" : " items")) + (root.pinnedCount ? " · " + root.pinnedCount + " pinned" : "")
            font.family: Theme.typography.mono
            font.pixelSize: 10
            color: Theme.colors.subtle
        }
        Rectangle {
            id: clearAction
            anchors.right: parent.right
            anchors.rightMargin: 22
            y: 15
            width: panel.width >= 540 ? 154 : 82
            height: 28
            visible: !root.confirmClear
            radius: Theme.radii.small
            color: root.clearSelected ? Theme.colors.selected : (clearActionMouse.containsMouse ? Theme.colors.hover : "transparent")
            border.width: root.clearSelected ? 1 : 0
            border.color: Theme.colors.selectionBorder
            opacity: history.entries.length && !history.busy ? 1 : 0.45
            Accessible.role: Accessible.Button
            Accessible.name: "Clear clipboard history"
            Accessible.onPressAction: root.beginClear()
            Text {
                anchors.centerIn: parent
                text: panel.width >= 540 ? "Clear history · Ctrl⇧Del" : "Clear history"
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
            onTextChanged: root.queryChanged()
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
            onContentYChanged: {
                if (!root.viewportRestorePending)
                    root.rememberSelection();
            }
            delegate: Item {
                required property var modelData
                required property int index
                readonly property int groupGap: index > 0 && !modelData.pinned && root.results[index - 1]?.pinned ? 12 : 0
                width: list.width
                height: row.implicitHeight + groupGap
                ClipboardRow {
                    id: row
                    y: parent.groupGap
                    width: parent.width
                    entry: parent.modelData
                    selected: !root.clearSelected && parent.index === root.selectedIndex
                    actionFocus: selected ? root.actionFocus : -1
                    busy: history.busy
                    onPinned: {
                        root.clearSelected = false;
                        root.selectIndex(parent.index);
                        root.pinSelected();
                    }
                    onPlainCopied: {
                        root.clearSelected = false;
                        root.selectIndex(parent.index);
                        root.copyPlainSelected();
                    }
                    onChosen: {
                        root.clearSelected = false;
                        root.actionFocus = 0;
                        root.selectIndex(parent.index);
                        root.launchSelected();
                    }
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
                text: history.entries.length ? "Search text, filenames or file types." : "Copy text, images, files or folders to save them here."
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
                text: root.clearChoice === 2 ? "Clear everything?" : "Clear clipboard history?"
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: 19
            }
            Text {
                y: 34
                width: parent.width
                text: root.clearChoice === 2 ? "All saved items, including pins, will be removed." : (root.pinnedCount ? "Unpinned items are removed. Pins stay saved." : "This removes the saved history.")
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }
            Rectangle {
                id: cancelButton
                y: 78
                width: (parent.width - (root.pinnedCount ? 16 : 12)) / root.clearChoiceCount
                height: 44
                radius: Theme.radii.medium
                color: root.clearChoice === 0 ? Theme.colors.selected : (cancelMouse.containsMouse ? Theme.colors.hover : "transparent")
                border.width: 1
                border.color: root.clearChoice === 0 ? Theme.colors.selectionBorder : Theme.colors.border
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
                    font.pixelSize: 12
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
                x: cancelButton.width + (root.pinnedCount ? 8 : 12)
                y: 78
                width: cancelButton.width
                height: 44
                radius: Theme.radii.medium
                color: root.clearChoice === 1 ? Theme.colors.selected : (clearMouse.containsMouse ? Theme.colors.hover : "transparent")
                border.width: 1
                border.color: root.clearChoice === 1 ? Theme.colors.selectionBorder : Theme.colors.border
                enabled: !history.busy
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: "Clear unpinned history"
                Accessible.onPressAction: {
                    root.clearChoice = 1;
                    root.acceptClear();
                }
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: event => root.handleKey(event)
                Text {
                    anchors.centerIn: parent
                    text: "Clear history"
                    color: Theme.colors.error
                    font.family: Theme.typography.family
                    font.pixelSize: 12
                }
                MouseArea {
                    id: clearMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.clearChoice = 1;
                        root.acceptClear();
                    }
                }
            }
            Rectangle {
                id: clearAllButton
                anchors.right: parent.right
                y: 78
                width: cancelButton.width
                height: 44
                visible: root.pinnedCount > 0
                radius: Theme.radii.medium
                color: root.clearChoice === 2 ? Theme.colors.selected : (clearAllMouse.containsMouse ? Theme.colors.hover : "transparent")
                border.width: 1
                border.color: root.clearChoice === 2 ? Theme.colors.selectionBorder : Theme.colors.border
                enabled: !history.busy
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: "Clear everything including pinned items"
                Accessible.onPressAction: {
                    root.clearChoice = 2;
                    root.acceptClear();
                }
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: event => root.handleKey(event)
                Text {
                    anchors.centerIn: parent
                    text: "Clear everything"
                    color: Theme.colors.error
                    font.family: Theme.typography.family
                    font.pixelSize: 12
                }
                MouseArea {
                    id: clearAllMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.clearChoice = 2;
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
            navigationKeys: root.confirmClear ? "Tab" : "↑ ↓ / Tab"
            navigationLabel: root.confirmClear ? "choose" : "navigate"
            escapeLabel: root.confirmClear ? "cancel" : "close"
            hintsVisible: !history.error
            actionText: root.confirmClear ? (["Cancel", "Clear history", "Clear everything"][root.clearChoice]) : (history.busy ? "Working…" : (root.clearSelected ? "Clear history" : (root.actionFocus === 1 ? (root.results[root.selectedIndex]?.pinned ? "Unpin · Ctrl⇧P" : "Pin · Ctrl⇧P") : (root.actionFocus === 2 ? "Copy plain · Ctrl⇧C" : "Copy & close"))))
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
