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
    property var viewport: null
    readonly property int columnCount: 4
    property var previewUrls: ({})
    property var previewFailures: ({})
    property string previewPath: ""
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
        else {
            inventory.running = false;
            previewRequest.running = false;
            previewTimer.stop();
        }
    }
    function refresh(): void {
        if (active && !inventory.running && !busy)
            inventory.running = true;
    }
    function schedulePreviews(): void {
        if (active)
            previewTimer.restart();
    }
    function loadVisiblePreview(): void {
        if (!active || previewRequest.running || !entries.length)
            return;
        const tileHeight = ((grid.width - grid.spacing * (root.columnCount - 1)) / root.columnCount) * 0.6;
        const rowHeight = tileHeight + grid.spacing;
        const offset = viewport ? grid.mapToItem(viewport, 0, 0).y : 0;
        const first = viewport ? Math.max(0, Math.floor((-offset) / rowHeight) - 1) * root.columnCount : 0;
        const last = viewport ? Math.min(entries.length, (Math.ceil((viewport.height - offset) / rowHeight) + 1) * root.columnCount) : entries.length;
        for (let i = first; i < last; ++i) {
            const path = entries[i].path;
            if (!previewUrls[path] && !previewFailures[path]) {
                previewPath = path;
                previewRequest.command = [backend, "--root", Quickshell.env("DF_FOUNDATION_ROOT"), "wallpapers", "preview", path];
                previewRequest.running = true;
                return;
            }
        }
    }
    Timer {
        id: previewTimer
        interval: 35
        onTriggered: root.loadVisiblePreview()
    }
    Connections {
        target: root.viewport
        function onContentYChanged(): void {
            root.schedulePreviews();
        }
        function onHeightChanged(): void {
            root.schedulePreviews();
        }
    }
    ControlRequest {
        id: previewRequest
        onFinished: code => {
            if (!root.active)
                return;
            const values = Object.assign({}, code === 0 && response.url ? root.previewUrls : root.previewFailures);
            values[root.previewPath] = code === 0 && response.url ? response.url : true;
            if (code === 0 && response.url)
                root.previewUrls = values;
            else
                root.previewFailures = values;
            root.schedulePreviews();
        }
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
                root.previewUrls = ({});
                root.previewFailures = ({});
                root.schedulePreviews();
            } else
                root.failed(failure || "Could not read wallpapers.");
        }
    }
    ControlRequest {
        id: apply
        expectJson: false
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
        columns: root.columnCount
        spacing: 12
        Repeater {
            id: tiles
            model: root.entries
            delegate: ControlButton {
                id: tile
                required property var modelData
                required property int index
                objectName: "wallpaper-" + index
                width: (grid.width - grid.spacing * (root.columnCount - 1)) / root.columnCount
                height: width * 0.6
                enabled: root.active && !root.busy
                readonly property bool selected: root.currentPath === modelData.path
                readonly property bool nearViewport: {
                    if (!root.viewport)
                        return true;
                    const scrollY = root.viewport.contentY;
                    const top = tile.mapToItem(root.viewport.contentItem, 0, 0).y - scrollY;
                    return top + height >= -height && top <= root.viewport.height + height;
                }
                Accessible.name: modelData.name
                Accessible.role: Accessible.RadioButton
                Accessible.checkable: true
                Accessible.checked: selected
                onClicked: root.choose(modelData)
                Keys.onPressed: event => {
                    if (tile.activateKey(event))
                        return;
                    if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
                        return;
                    const direction = Material.direction(event.key);
                    if (direction) {
                        const next = index + direction[0] + direction[1] * root.columnCount;
                        if (next >= 0 && next < tiles.count && (!direction[0] || Math.floor(next / root.columnCount) === Math.floor(index / root.columnCount)))
                            tiles.itemAt(next).forceActiveFocus(Qt.TabFocusReason);
                        else if (direction[1] < 0 && index < root.columnCount)
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
                        source: root.active && tile.nearViewport ? root.previewUrls[tile.modelData.path] || "" : ""
                        sourceSize.width: Math.ceil(tile.width * (Screen.devicePixelRatio || 1))
                        sourceSize.height: Math.ceil(tile.height * (Screen.devicePixelRatio || 1))
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }
                    Text {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: image.status === Image.Error || !!root.previewFailures[tile.modelData.path]
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
