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
    onResultsChanged: selectedIndex = 0
    function present(): void {
        closing = false;
        exitAnimation.stop();
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
        if (results.length > 0)
            history.request("copy", results[selectedIndex].id);
    }
    function navigate(delta: int): void {
        if (!results.length)
            return;
        selectedIndex = Math.max(0, Math.min(results.length - 1, selectedIndex + delta));
        list.positionViewAtIndex(selectedIndex, ListView.Contain);
    }
    function setQuery(value: string): void {
        search.text = value;
    }
    function snapshot(): var {
        return {
            inputFocused: search.activeFocus,
            closing: root.closing,
            query: search.text,
            count: results.length,
            selected: results[selectedIndex]?.id ?? null,
            error: history.error,
            loading: history.loading,
            confirmClear: confirmClear,
            visible: !closing
        };
    }
    Component.onCompleted: {
        lifecycle.clipboardAlive = true;
        entered = true;
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
        width: Math.min(Theme.dimensions.launcherWidth, root.width - 48)
        height: Math.min(182 + Math.max(2, Math.min(6, root.results.length)) * 66, root.height - 64)
        Behavior on height {
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
            anchors.rightMargin: 108
            y: 23
            text: root.results.length + (search.text ? (root.results.length === 1 ? " match" : " matches") : " available")
            font.family: Theme.typography.mono
            font.pixelSize: 10
            color: Theme.colors.subtle
        }
        Text {
            anchors.right: parent.right
            anchors.rightMargin: 28
            y: 23
            text: "Clear all"
            color: Theme.colors.muted
            font.family: Theme.typography.family
            font.pixelSize: 11
            opacity: history.entries.length ? 1 : 0.45
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                enabled: history.entries.length > 0
                onClicked: root.confirmClear = true
            }
        }
        Image {
            x: 28
            y: 62
            width: 24
            height: 24
            source: "../assets/search.svg"
        }
        SearchInput {
            id: search
            x: 68
            y: 57
            width: panel.width - 96
            height: 38
            placeholderText: "Search clipboard…"
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Escape) {
                    if (root.confirmClear)
                        root.confirmClear = false;
                    else
                        root.dismiss();
                } else if (event.key === Qt.Key_Delete && (event.modifiers & Qt.ControlModifier)) {
                    if (root.results.length)
                        history.request("delete", root.results[root.selectedIndex].id);
                } else if (event.key === Qt.Key_Down || (event.key === Qt.Key_N && (event.modifiers & Qt.ControlModifier)))
                    root.navigate(1);
                else if (event.key === Qt.Key_Up || (event.key === Qt.Key_P && (event.modifiers & Qt.ControlModifier)))
                    root.navigate(-1);
                else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                    if (root.confirmClear) {
                        history.request("clear", "");
                        root.confirmClear = false;
                    } else
                        root.launchSelected();
                else if (event.key === Qt.Key_Tab)
                    root.navigate((event.modifiers & Qt.ShiftModifier) ? -1 : 1);
                else
                    return;
                event.accepted = true;
            }
        }
        Rectangle {
            x: 28
            y: 110
            width: panel.width - 56
            height: 1
            color: Theme.colors.border
        }
        ListView {
            id: list
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
                selected: index === root.selectedIndex
                onRemoved: history.request("delete", modelData.id)
                onChosen: {
                    root.selectedIndex = index;
                    root.launchSelected();
                }
            }
        }
        Column {
            visible: root.results.length === 0
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
        Rectangle {
            visible: root.confirmClear
            x: list.x
            y: list.y
            width: list.width
            height: list.height
            radius: Theme.radii.medium
            color: Theme.colors.elevated
            MouseArea {
                anchors.fill: parent
            }
            Column {
                anchors.centerIn: parent
                spacing: 12
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Clear clipboard history?"
                    color: Theme.colors.foreground
                    font.family: Theme.typography.family
                    font.pixelSize: 16
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "This removes all saved items."
                    color: Theme.colors.muted
                    font.family: Theme.typography.family
                    font.pixelSize: 12
                }
                Row {
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 24
                    Text {
                        text: "Cancel"
                        color: Theme.colors.muted
                        font.family: Theme.typography.family
                        font.pixelSize: 13
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.confirmClear = false
                        }
                    }
                    Text {
                        text: "Clear history"
                        color: Theme.colors.error
                        font.family: Theme.typography.family
                        font.pixelSize: 13
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                history.request("clear", "");
                                root.confirmClear = false;
                            }
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
            color: Theme.colors.error
            font.family: Theme.typography.family
            font.pixelSize: 11
            elide: Text.ElideRight
        }
        Rectangle {
            x: 28
            y: panel.height - 48
            width: panel.width - 56
            height: 1
            color: Theme.colors.border
        }
        Row {
            visible: !history.error
            x: 28
            y: panel.height - 35
            spacing: 7
            Keycap {
                label: "↑ ↓"
            }
            Text {
                text: "navigate"
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 11
                anchors.verticalCenter: parent.verticalCenter
            }
            Item {
                width: 8
                height: 1
            }
            Keycap {
                label: "esc"
            }
            Text {
                text: "close"
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 11
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        Row {
            anchors.right: parent.right
            anchors.rightMargin: 28
            y: panel.height - 35
            spacing: 7
            Text {
                text: root.confirmClear ? "Enter to clear" : (history.busy ? "Copying…" : "Copy & close")
                color: Theme.colors.accent
                font.family: Theme.typography.family
                font.pixelSize: 11
                anchors.verticalCenter: parent.verticalCenter
            }
            Keycap {
                label: "↵"
            }
        }
    }
}
