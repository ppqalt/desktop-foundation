pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import "../components"
import "../services"
import "../theme"

// qmllint disable uncreatable-type
// PanelWindow becomes creatable in the Quickshell runtime.
PopupWindow {
    id: root
    required property var lifecycle
    surfaceNamespace: "desktop-foundation-clipboard"
    ClipboardHistory {
        id: history
        onAboutToUpdate: root.captureViewport()
        onCompleted: action => {
            if (action === "copy" || action === "copy-plain")
                Qt.callLater(() => root.dismiss());
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
    readonly property bool confirmationShown: contentPane.currentKey === "confirmation"
    readonly property bool changingView: !contentPane.acceptsInput
    property bool clearSelected: false
    // 0 = copy, 1 = pin. Plain-text copy remains a keyboard shortcut.
    property int actionFocus: 0
    // Confirmation always starts on Cancel. Clearing pins is a distinct action.
    property int clearChoice: 0
    property int confirmationFocusReason: Qt.OtherFocusReason
    readonly property int clearChoiceCount: pinnedCount ? 3 : 2
    onClearChoiceCountChanged: {
        if (clearChoice >= clearChoiceCount)
            clearChoice = 0;
    }
    onConfirmClearChanged: {
        contentPane.request(confirmClear ? "confirmation" : "overview", confirmClear);
    }
    onClearChoiceChanged: {
        if (confirmClear && confirmationShown && !changingView)
            focusClearChoice();
    }
    onResultsChanged: reconcileSelection()
    function focusClearChoice(): void {
        if (closing || superseded || !confirmationShown || changingView)
            return;
        if (clearChoice === 1)
            Material.restoreFocus(clearButton, confirmationFocusReason);
        else if (clearChoice === 2)
            Material.restoreFocus(clearAllButton, confirmationFocusReason);
        else
            Material.restoreFocus(cancelButton, confirmationFocusReason);
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
        selectedIndex = Material.listIndex(index, results.length);
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
        else if (actionFocus > 1)
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
        beginPresentation();
        confirmClear = false;
        clearSelected = false;
        actionFocus = 0;
    }
    function dismiss(): void {
        if (history.busy || closing)
            return;
        beginDismissal();
    }
    function plainAvailable(): bool {
        const entry = results[selectedIndex];
        return !!entry && (entry.kind === "rich" || (!entry.kind && ["text/html", "text/rtf", "application/rtf"].includes(entry.mime)));
    }
    function launchSelected(): void {
        if (closing || changingView || confirmClear || history.busy)
            return;
        if (clearSelected)
            beginClear(Qt.TabFocusReason);
        else if (results.length > 0) {
            if (actionFocus === 1)
                pinSelected();
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
        if (clearSelected && delta < 0) {
            clearSelected = false;
            search.forceActiveFocus();
            return;
        }
        panel.forceActiveFocus();
        actionFocus = 0;
        if (history.entries.length && (!results.length || (!clearSelected && delta < 0 && selectedIndex === 0)))
            clearSelected = true;
        else
            navigate(delta);
    }
    function tabNavigate(delta: int): void {
        if (search.activeFocus && delta > 0) {
            panel.forceActiveFocus();
            return;
        }
        if (clearSelected && delta < 0) {
            clearSelected = false;
            search.forceActiveFocus();
            return;
        }
        if (!history.entries.length) {
            if (delta < 0)
                search.forceActiveFocus();
            return;
        }
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
            const lastAction = 1;
            if (actionFocus + delta >= 0 && actionFocus + delta <= lastAction)
                actionFocus += delta;
            else if (delta < 0 && selectedIndex === 0) {
                clearSelected = true;
                actionFocus = 0;
            } else if (delta > 0 && selectedIndex === results.length - 1) {
                return;
            } else {
                selectIndex(selectedIndex + delta);
                actionFocus = delta > 0 ? 0 : 1;
            }
        }
        if (!clearSelected) {
            list.positionViewAtIndex(selectedIndex, ListView.Contain);
            rememberSelection();
        }
    }
    function beginClear(reason = Qt.OtherFocusReason): void {
        if (closing || changingView || confirmClear || history.busy || !history.entries.length)
            return;
        clearChoice = 0;
        confirmationFocusReason = reason;
        confirmClear = true;
    }
    function acceptClear(): void {
        if (!confirmClear || changingView || history.busy)
            return;
        if (clearChoice === 1)
            history.request("clear", "");
        else if (clearChoice === 2 && pinnedCount)
            history.request("clear-all", "");
        confirmClear = false;
        clearSelected = false;
        actionFocus = 0;
    }
    function activateClearChoice(): void {
        if (!confirmClear || changingView || history.busy)
            return;
        const action = [cancelButton, clearButton, clearAllButton][clearChoice];
        if (action?.visible && action.enabled)
            action.clicked();
    }
    function deleteSelected(): void {
        if (!closing && !changingView && !confirmClear && !clearSelected && !history.busy && results.length) {
            rememberSelection();
            history.request("delete", results[selectedIndex].id);
        }
    }
    function pinSelected(): void {
        if (!closing && !changingView && !confirmClear && !clearSelected && !history.busy && results.length) {
            rememberSelection();
            history.request(results[selectedIndex].pinned ? "unpin" : "pin", results[selectedIndex].id);
        }
    }
    function copyPlainSelected(): void {
        const entry = results[selectedIndex];
        const textual = !!entry && (entry.kind === "text" || plainAvailable() || (!entry.kind && (entry.mime || "").startsWith("text/")));
        if (!closing && !changingView && !confirmClear && !clearSelected && !history.busy && textual)
            history.request("copy-plain", entry.id);
    }
    function handleKey(event: var): void {
        const control = !!(event.modifiers & Qt.ControlModifier);
        const shift = !!(event.modifiers & Qt.ShiftModifier);
        const editing = search.activeFocus;
        const activate = Material.isActivation(event, editing);
        const remove = event.key === Qt.Key_Delete && control;
        const pin = event.key === Qt.Key_P && control && shift;
        const plain = event.key === Qt.Key_C && control && shift;
        if (root.closing || Material.repeatSuppressed(event, editing) || ((activate || remove || pin || plain || (root.confirmClear && event.key === Qt.Key_Space)) && event.isAutoRepeat)) {
            event.accepted = true;
            return;
        }
        if (root.changingView) {
            if (event.key === Qt.Key_Escape)
                root.confirmClear = false;
            event.accepted = true;
            return;
        }
        if (root.confirmClear) {
            if ([Qt.Key_Tab, Qt.Key_Backtab, Qt.Key_Left, Qt.Key_Right, Qt.Key_Up, Qt.Key_Down, Qt.Key_A, Qt.Key_W, Qt.Key_D, Qt.Key_S, Qt.Key_Home, Qt.Key_End].includes(event.key))
                root.confirmationFocusReason = Qt.TabFocusReason;
            if (event.key === Qt.Key_Escape)
                root.confirmClear = false;
            else if (activate || event.key === Qt.Key_Space)
                root.activateClearChoice();
            else if (event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && shift) || event.key === Qt.Key_Left || event.key === Qt.Key_Up || [Qt.Key_A, Qt.Key_W].includes(event.key))
                root.clearChoice = (root.clearChoice + root.clearChoiceCount - 1) % root.clearChoiceCount;
            else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Right || event.key === Qt.Key_Down || [Qt.Key_D, Qt.Key_S].includes(event.key))
                root.clearChoice = (root.clearChoice + 1) % root.clearChoiceCount;
            else if (event.key === Qt.Key_Home)
                root.clearChoice = 0;
            else if (event.key === Qt.Key_End)
                root.clearChoice = root.clearChoiceCount - 1;
            // Modal input never reaches the hidden query or covered items.
            if (root.confirmClear)
                root.focusClearChoice();
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
            root.beginClear(Qt.TabFocusReason);
        else if (remove)
            root.deleteSelected();
        else if (event.key === Qt.Key_Down || (!editing && event.key === Qt.Key_S) || (event.key === Qt.Key_N && control))
            root.keyboardNavigate(1);
        else if (event.key === Qt.Key_Up || (!editing && event.key === Qt.Key_W) || (event.key === Qt.Key_P && control))
            root.keyboardNavigate(-1);
        else if (event.key === Qt.Key_PageDown || event.key === Qt.Key_PageUp)
            root.navigate((event.key === Qt.Key_PageDown ? 1 : -1) * Math.max(1, Math.floor(list.height / 76)));
        else if (activate)
            root.launchSelected();
        else if (event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && shift))
            root.tabNavigate(-1);
        else if (event.key === Qt.Key_Tab)
            root.tabNavigate(1);
        else if (root.actionFocus && (event.key === Qt.Key_Left || event.key === Qt.Key_Right || (!editing && [Qt.Key_A, Qt.Key_D].includes(event.key))))
            root.tabNavigate([Qt.Key_Left, Qt.Key_A].includes(event.key) ? -1 : 1);
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
            confirmationShown: confirmationShown,
            changingView: changingView,
            viewMoving: contentPane.running,
            clearSelected: clearSelected,
            clearChoice: clearChoice,
            visible: !closing
        };
    }
    Component.onCompleted: {
        lifecycle.clipboardAlive = true;
    }
    Component.onDestruction: lifecycle.clipboardAlive = false
    motionItem: panel
    initialFocusItem: search
    closeBlocked: history.busy
    canDispose: !history.busy
    onDisposalReady: root.lifecycle.clipboardEnabled = false
    onCloseRequested: root.dismiss()
    PopupCard {
        id: panel
        Keys.onPressed: event => root.handleKey(event)
        width: Material.popupWidth(root.confirmClear ? 440 : Theme.dimensions.launcherWidth, root.width)
        height: Material.popupHeight(root.confirmClear ? Math.max(268, confirmation.implicitHeight + 112) : 144 + (history.loading ? 6 : Math.max(2, Math.min(6, root.results.length))) * 76 + (root.results.slice(0, 6).some(entry => entry.pinned) && root.results.slice(0, 6).some(entry => !entry.pinned) ? 12 : 0), root.height)
        Behavior on width {
            // Initial output geometry uses the shared fade/scale entrance.
            // Keep the existing confirmation morph once opened (or requested).
            enabled: root.confirmClear || (root.entered && panel.opacity === 1 && !root.closing)
            SurfaceAnimation {
                duration: Theme.timing.normal
                easing.type: Theme.easing
            }
        }
        Behavior on height {
            enabled: root.confirmClear || (root.entered && panel.opacity === 1 && !root.closing)
            SurfaceAnimation {
                duration: Theme.timing.normal
                easing.type: Theme.easing
            }
        }
        presented: root.entered
        dismissing: root.closing
        // Capture blank panel clicks without moving keyboard focus.
        MouseArea {
            anchors.fill: parent
        }
        PagePane {
            id: contentPane
            objectName: "clipboardPages"
            anchors.fill: parent
            focusAllowed: !root.closing && !root.superseded
            onCancelled: {
                if (!root.closing && !root.superseded)
                    search.forceActiveFocus();
            }
            onCommitted: key => {
                if (root.closing || root.superseded)
                    return;
                if (key === "confirmation")
                    root.focusClearChoice();
                else
                    search.forceActiveFocus();
            }
            ControlAction {
                id: clearAction
                objectName: "clipboardOpenClear"
                anchors.right: parent.right
                anchors.rightMargin: 22
                y: 6
                width: 120
                height: 48
                visible: !root.confirmationShown
                compact: true
                activeFocusOnTab: false
                text: "Clear history"
                marked: root.clearSelected
                enabled: history.entries.length > 0 && !history.busy
                Accessible.name: "Clear clipboard history"
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: event => root.handleKey(event)
                onPointerFocused: {
                    root.clearSelected = true;
                    root.actionFocus = 0;
                }
                onClicked: root.beginClear(visualFocus ? Qt.TabFocusReason : Qt.MouseFocusReason)
            }
            Image {
                visible: !root.confirmationShown
                x: 28
                y: 18
                width: 24
                height: 24
                source: "../assets/search.svg"
            }
            SearchInput {
                id: search
                visible: !root.confirmationShown
                x: 68
                y: 11
                width: panel.width - 244
                height: 38
                placeholderText: "Search clipboard…"
                readOnly: root.confirmClear
                onActiveFocusChanged: {
                    if (activeFocus && !root.confirmClear) {
                        root.actionFocus = 0;
                        root.clearSelected = false;
                    }
                }
                onTextChanged: root.queryChanged()
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: event => root.handleKey(event)
            }
            Rectangle {
                visible: !root.confirmationShown
                x: 28
                y: 72
                width: panel.width - 56
                height: 1
                color: Material.colors.outline_variant
            }
            ListView {
                id: list
                visible: !root.confirmationShown
                enabled: !root.confirmationShown
                SelectionWheel {
                    enabled: !root.confirmClear
                    onStepped: delta => root.navigate(delta)
                }
                x: 16
                y: 88
                width: panel.width - 32
                height: panel.height - 148
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
                visible: !root.confirmationShown && root.results.length === 0
                anchors.centerIn: list
                spacing: 10
                WidgetStatus {
                    anchors.horizontalCenter: parent.horizontalCenter
                    status: history.loading ? "loading" : "empty"
                    message: history.loading ? "Loading clipboard…" : (history.entries.length ? "No matching items" : "Clipboard empty")
                    font.pixelSize: Material.type.bodyLarge
                }
            }
            FocusScope {
                id: confirmation
                visible: root.confirmationShown
                x: 28
                y: 64
                width: panel.width - 56
                implicitHeight: confirmationBody.implicitHeight
                Keys.priority: Keys.BeforeItem
                Keys.onPressed: event => root.handleKey(event)
                ColumnLayout {
                    id: confirmationBody
                    width: parent.width
                    spacing: 8
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: root.clearChoice === 2 ? "Clear everything?" : "Clear clipboard history?"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleLarge
                    }
                    Text {
                        Layout.fillWidth: true
                        text: root.clearChoice === 2 ? "All saved items, including pins, will be removed." : (root.pinnedCount ? "Unpinned items are removed. Pins stay saved." : "This removes the saved history.")
                        color: Material.colors.on_surface_variant
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                        wrapMode: Text.WordWrap
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columnSpacing: 8
                        rowSpacing: 4
                        columns: confirmation.width >= cancelButton.minimumContentWidth + clearButton.minimumContentWidth + (root.pinnedCount ? clearAllButton.minimumContentWidth + 16 : 8) ? root.clearChoiceCount : 1
                        ControlAction {
                            id: cancelButton
                            objectName: "clipboardCancelClear"
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            Layout.minimumWidth: 0
                            compact: true
                            text: "Cancel"
                            marked: root.clearChoice === 0
                            enabled: !history.busy
                            Keys.priority: Keys.BeforeItem
                            Keys.onPressed: event => root.handleKey(event)
                            onPointerFocused: {
                                root.confirmationFocusReason = Qt.MouseFocusReason;
                                root.clearChoice = 0;
                            }
                            onClicked: root.confirmClear = false
                        }
                        ControlAction {
                            id: clearButton
                            objectName: "clipboardClearUnpinned"
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            Layout.minimumWidth: 0
                            compact: true
                            destructive: true
                            text: "Clear history"
                            marked: root.clearChoice === 1
                            enabled: !history.busy
                            Accessible.name: "Clear unpinned history"
                            Keys.priority: Keys.BeforeItem
                            Keys.onPressed: event => root.handleKey(event)
                            onPointerFocused: {
                                root.confirmationFocusReason = Qt.MouseFocusReason;
                                root.clearChoice = 1;
                            }
                            onClicked: {
                                root.clearChoice = 1;
                                root.acceptClear();
                            }
                        }
                        ControlAction {
                            id: clearAllButton
                            objectName: "clipboardClearAll"
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            Layout.minimumWidth: 0
                            visible: root.pinnedCount > 0
                            compact: true
                            destructive: true
                            text: "Clear all"
                            marked: root.clearChoice === 2
                            enabled: !history.busy
                            Accessible.name: "Clear everything including pinned items"
                            Keys.priority: Keys.BeforeItem
                            Keys.onPressed: event => root.handleKey(event)
                            onPointerFocused: {
                                root.confirmationFocusReason = Qt.MouseFocusReason;
                                root.clearChoice = 2;
                            }
                            onClicked: {
                                root.clearChoice = 2;
                                root.acceptClear();
                            }
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
                color: Material.colors.error
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
                hintsVisible: false
                actionText: history.busy ? "Working…" : ""
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
}
