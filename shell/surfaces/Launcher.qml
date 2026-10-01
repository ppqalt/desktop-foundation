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
    WlrLayershell.namespace: "desktop-foundation-launcher"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    color: "transparent"
    Applications {
        id: applications
    }
    readonly property var results: applications.search(search.text)
    property int selectedIndex: 0
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
        if (results.length > 0 && applications.launch(results[selectedIndex]))
            dismiss();
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
            visible: !closing
        };
    }
    Component.onCompleted: {
        lifecycle.launcherAlive = true;
        entered = true;
        search.forceActiveFocus();
    }
    Component.onDestruction: lifecycle.launcherAlive = false
    SequentialAnimation {
        id: exitAnimation
        PauseAnimation {
            duration: Theme.timing.exit
        }
        ScriptAction {
            script: root.lifecycle.launcherEnabled = false
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
            onStepped: delta => root.navigate(delta)
        }
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
            text: "APPLICATIONS"
            font.family: Theme.typography.mono
            font.pixelSize: 10
            font.letterSpacing: 2
            color: Theme.colors.muted
        }
        Text {
            anchors.right: parent.right
            anchors.rightMargin: 28
            y: 23
            text: root.results.length + (search.text ? (root.results.length === 1 ? " match" : " matches") : " available")
            font.family: Theme.typography.mono
            font.pixelSize: 10
            color: Theme.colors.subtle
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
            placeholderText: "Search applications…"
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Escape)
                    root.dismiss();
                else if (event.key === Qt.Key_Down || (event.key === Qt.Key_N && (event.modifiers & Qt.ControlModifier)))
                    root.navigate(1);
                else if (event.key === Qt.Key_Up || (event.key === Qt.Key_P && (event.modifiers & Qt.ControlModifier)))
                    root.navigate(-1);
                else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
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
            SelectionWheel {
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
            delegate: ApplicationRow {
                required property var modelData
                required property int index
                width: list.width
                entry: modelData
                selected: index === root.selectedIndex
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
                text: "No matching applications"
                color: Theme.colors.foreground
                font.family: Theme.typography.family
                font.pixelSize: 16
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Try a shorter name or another keyword."
                color: Theme.colors.muted
                font.family: Theme.typography.family
                font.pixelSize: 12
            }
        }
        Rectangle {
            x: 28
            y: panel.height - 48
            width: panel.width - 56
            height: 1
            color: Theme.colors.border
        }
        Row {
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
                text: "Open application"
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
