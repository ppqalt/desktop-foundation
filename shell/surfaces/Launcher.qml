pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "../components"
import "../services"
import "../theme"

// qmllint disable uncreatable-type
// PanelWindow becomes creatable in the Quickshell runtime.
PopupWindow {
    id: root
    required property var lifecycle
    surfaceNamespace: "desktop-foundation-launcher"
    Applications {
        id: applications
    }
    readonly property var results: applications.search(search.text)
    property int selectedIndex: 0
    onResultsChanged: selectedIndex = 0
    function present(): void {
        beginPresentation();
    }
    function dismiss(): void {
        if (closing)
            return;
        beginDismissal();
    }
    function launchSelected(): void {
        if (!closing && results.length > 0 && applications.launch(results[selectedIndex]))
            dismiss();
    }
    function navigate(delta: int): void {
        if (!results.length)
            return;
        selectedIndex = Material.listIndex(selectedIndex + delta, results.length);
        list.positionViewAtIndex(selectedIndex, ListView.Contain);
    }
    function handleKey(event): void {
        const editing = search.activeFocus;
        const activate = Material.isActivation(event, editing);
        if (closing || Material.repeatSuppressed(event, editing)) {
            event.accepted = true;
            return;
        }
        if (event.key === Qt.Key_Escape)
            dismiss();
        else if (event.key === Qt.Key_Down || (!editing && event.key === Qt.Key_S) || (event.key === Qt.Key_N && (event.modifiers & Qt.ControlModifier))) {
            navigate(1);
            panel.forceActiveFocus();
        } else if (event.key === Qt.Key_Up || (!editing && event.key === Qt.Key_W) || (event.key === Qt.Key_P && (event.modifiers & Qt.ControlModifier))) {
            if (selectedIndex === 0)
                search.forceActiveFocus();
            else
                navigate(-1);
        } else if (event.key === Qt.Key_PageDown || event.key === Qt.Key_PageUp)
            navigate((event.key === Qt.Key_PageDown ? 1 : -1) * Math.max(1, Math.floor(list.height / 76)));
        else if (activate)
            launchSelected();
        else if (event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))) {
            if (selectedIndex === 0)
                search.forceActiveFocus();
            else
                navigate(-1);
        } else if (event.key === Qt.Key_Tab) {
            if (!editing)
                navigate(1);
            panel.forceActiveFocus();
        } else
            return;
        event.accepted = true;
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
    }
    Component.onDestruction: lifecycle.launcherAlive = false
    motionItem: panel
    initialFocusItem: search
    onDisposalReady: root.lifecycle.launcherEnabled = false
    onCloseRequested: root.dismiss()
    PopupCard {
        id: panel
        Keys.onPressed: event => root.handleKey(event)
        width: Material.popupWidth(Theme.dimensions.launcherWidth, root.width)
        height: Material.popupHeight(144 + Math.max(2, Math.min(6, root.results.length)) * 76, root.height)
        Behavior on height {
            enabled: root.entered && panel.opacity === 1 && !root.closing
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
        Image {
            x: 28
            y: 18
            width: 24
            height: 24
            source: "../assets/search.svg"
        }
        SearchInput {
            id: search
            x: 68
            y: 11
            width: panel.width - 96
            height: 38
            placeholderText: "Search applications…"
            Keys.onPressed: event => root.handleKey(event)
        }
        Rectangle {
            x: 28
            y: 72
            width: panel.width - 56
            height: 1
            color: Material.colors.outline_variant
        }
        ListView {
            id: list
            SelectionWheel {
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
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: 16
            }
        }
        SurfaceFooter {
            x: 28
            y: panel.height - 48
            width: panel.width - 56
            actionText: ""
            actionEnabled: !root.closing && root.results.length > 0
            onActivated: root.launchSelected()
            escapeEnabled: !root.closing
            onEscapeRequested: root.dismiss()
        }
    }
}
