pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import "../theme"

ColumnLayout {
    id: root
    required property string backend
    property bool active: true
    property var entries: []
    property string currentPath: ""
    property string requestedPath: ""
    readonly property bool busy: apply.running
    readonly property bool loading: inventory.running
    property bool initialized: false
    signal started
    signal failed(string message)
    signal settled
    spacing: 12
    Component.onCompleted: {
        initialized = true;
        refresh();
    }
    onActiveChanged: {
        if (!initialized)
            return;
        if (active)
            refresh();
        else
            inventory.running = false;
    }
    function refresh(): void {
        if (active && !inventory.running && !busy)
            inventory.running = true;
    }
    function choose(entry): void {
        if (!active || busy || entry.path === currentPath)
            return;
        requestedPath = entry.path;
        root.started();
        apply.command = [backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "theme-runtime", "wallpaper", entry.path];
        apply.running = true;
    }
    ControlRequest {
        id: inventory
        command: [root.backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "wallpapers", "list"]
        onFinished: code => {
            if (!root.active)
                return;
            if (code === 0 && Array.isArray(response.images)) {
                root.entries = response.images;
                root.currentPath = response.current || "";
            } else
                root.failed(failure || "Could not read wallpapers.");
        }
    }
    ControlRequest {
        id: apply
        onFinished: code => {
            if (code === 0)
                root.currentPath = root.requestedPath;
            else
                root.failed(failure || "Could not apply wallpaper.");
            root.requestedPath = "";
            root.settled();
        }
    }
    Text {
        text: "Wallpapers"
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.titleMedium
    }
    Grid {
        id: grid
        Layout.fillWidth: true
        columns: 3
        spacing: 12
        Repeater {
            id: tiles
            model: root.entries
            delegate: AbstractButton {
                id: tile
                required property var modelData
                required property int index
                objectName: "wallpaper-" + index
                width: (grid.width - 24) / 3
                height: width * 0.6
                activeFocusOnTab: true
                enabled: root.active && !root.busy
                readonly property bool selected: root.currentPath === modelData.path
                Accessible.name: modelData.name
                Accessible.role: Accessible.RadioButton
                Accessible.checkable: true
                Accessible.checked: selected
                onClicked: root.choose(modelData)
                Keys.onReturnPressed: event => {
                    if (!event.isAutoRepeat)
                        root.choose(modelData);
                    event.accepted = true;
                }
                Keys.onEnterPressed: event => {
                    if (!event.isAutoRepeat)
                        root.choose(modelData);
                    event.accepted = true;
                }
                Keys.onPressed: event => {
                    if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
                        return;
                    const direction = Material.direction(event.key);
                    if (direction) {
                        const next = index + direction[0] + direction[1] * 3;
                        if (next >= 0 && next < tiles.count && (!direction[0] || Math.floor(next / 3) === Math.floor(index / 3)))
                            tiles.itemAt(next).forceActiveFocus(Qt.TabFocusReason);
                        else if (direction[1] < 0 && index < 3)
                            Material.moveFocus(tile, 0, -1);
                        event.accepted = true;
                    }
                }
                background: Rectangle {
                    radius: Material.shape.medium
                    color: Material.colors.surface_container_high
                }
                contentItem: Item {
                    id: preview
                    Image {
                        id: image
                        anchors.fill: parent
                        source: root.active ? tile.modelData.url : ""
                        sourceSize.width: Math.ceil(tile.width * (Screen.devicePixelRatio || 1))
                        sourceSize.height: Math.ceil(tile.height * (Screen.devicePixelRatio || 1))
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }
                    Text {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: image.status === Image.Error
                        text: tile.modelData.name
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                    Rectangle {
                        anchors.fill: parent
                        color: Material.colors.primary
                        opacity: tile.down ? 0.12 : tile.hovered ? 0.08 : 0
                    }
                }
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: tile.activeFocus ? -3 : 0
                    radius: Material.shape.medium + (tile.activeFocus ? 3 : 0)
                    color: "transparent"
                    border.width: tile.activeFocus ? 3 : tile.selected ? 2 : 0
                    border.color: Material.colors.primary
                }
                Rectangle {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 8
                    width: 28
                    height: 28
                    radius: 14
                    visible: tile.selected
                    color: Material.colors.primary
                    ControlIcon {
                        anchors.centerIn: parent
                        width: 20
                        height: 20
                        name: "check"
                        color: Material.colors.on_primary
                    }
                }
                Text {
                    anchors.centerIn: parent
                    visible: root.busy && root.requestedPath === tile.modelData.path
                    text: "Applying…"
                    color: Material.colors.on_primary_container
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.labelLarge
                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: -8
                        z: -1
                        radius: 12
                        color: Material.colors.primary_container
                    }
                }
                ToolTip {
                    visible: tile.hovered
                    delay: 600
                    text: tile.modelData.name
                    padding: 8
                    background: Rectangle {
                        radius: Material.shape.small
                        color: Material.colors.surface_container_high
                    }
                    contentItem: Text {
                        text: tile.modelData.name
                        textFormat: Text.PlainText
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                }
            }
        }
    }
    Text {
        visible: !root.loading && root.entries.length === 0
        text: "No wallpapers"
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
    }
}
