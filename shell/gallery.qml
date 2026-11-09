import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "theme"
import "components"

// Developer preview of production components. This has no system backends.
ShellRoot {
    Window {
        id: window
        width: Number(Quickshell.env("DF_GALLERY_WIDTH")) || 760
        height: 640
        visible: true
        title: "Foundation component gallery"
        color: Material.colors.surface
        property string sliderEvent: "No changes yet"
        property int activationCount: 0
        property real sampleVolume: 65
        property bool sampleMuted: false
        property real sampleStart: 253
        property real sampleEnd: 300
        property real samplePlayhead: 253
        property bool mirroredPreview: false
        property real previewZoom: 1.0
        property var previewPalettes: JSON.parse(paletteData.text())
        property FileView paletteData: FileView {
            path: Quickshell.env("DF_FOUNDATION_ROOT") + "/shell/gallery-palettes.json"
            blockLoading: true
        }
        function selectPalette(index: int): void {
            const keys = ["", "violet-dark", "violet-light", "teal-dark", "teal-light"];
            if (index === 0) {
                Theme.generated = ({});
                Theme.reload();
                return;
            }
            const colors = previewPalettes[keys[index]];
            if (colors)
                Theme.generated = Object.assign({}, Theme.generated, {
                    material: colors
                });
        }
        property QtObject exampleRecording: QtObject {
            id: recordingExample
            property bool prepared: true
            property int bufferSeconds: 300
            property var state: ({
                    phase: "idle",
                    replay: false,
                    replay_status: "off"
                })
        }
        Flickable {
            id: galleryScroll
            anchors.fill: parent
            contentWidth: width
            contentHeight: content.implicitHeight * window.previewZoom + 48
            clip: true
            ScrollBar.vertical: FoundationScrollbar {}
            ColumnLayout {
                id: content
                x: 24
                y: 24
                width: Math.max(1, (parent.width - 48) / window.previewZoom)
                scale: window.previewZoom
                transformOrigin: Item.TopLeft
                LayoutMirroring.enabled: window.mirroredPreview
                LayoutMirroring.childrenInherit: true
                spacing: 16
                Text {
                    text: "Foundation components"
                    color: Material.colors.on_surface
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.headlineSmall
                }
                Text {
                    text: "Interactive examples use local sample state."
                    color: Material.colors.on_surface_variant
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.bodyMedium
                }
                ControlChoice {
                    Layout.fillWidth: true
                    Accessible.name: "Jump to gallery section"
                    textRole: "name"
                    model: [
                        {
                            name: "Jump to section"
                        },
                        {
                            name: "Actions and state"
                        },
                        {
                            name: "Input"
                        },
                        {
                            name: "Desktop examples"
                        }
                    ]
                    onActivated: index => {
                        const items = [null, actionsGroup, inputGroup, desktopGroup];
                        if (items[index])
                            galleryScroll.contentY = Math.min(galleryScroll.contentHeight - galleryScroll.height, Math.max(0, items[index].y * window.previewZoom));
                    }
                }
                ControlGroup {
                    Layout.fillWidth: true
                    Text {
                        text: "Preview settings"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ControlChoice {
                        id: paletteChoice
                        Layout.fillWidth: true
                        Accessible.name: "Preview palette"
                        textRole: "name"
                        model: [
                            {
                                name: "Current wallpaper"
                            },
                            {
                                name: "Violet dark"
                            },
                            {
                                name: "Violet light"
                            },
                            {
                                name: "Teal dark"
                            },
                            {
                                name: "Teal light"
                            }
                        ]
                        onActivated: index => window.selectPalette(index)
                    }
                    RowLayout {
                        Text {
                            text: "Reduced motion (preview only)"
                            color: Material.colors.on_surface
                            font.family: Theme.typography.family
                            font.pixelSize: Material.type.bodyMedium
                            Layout.fillWidth: true
                        }
                        ControlSwitch {
                            id: reducedMotionSwitch
                            text: "Reduced motion"
                            marked: Theme.reducedMotion
                            onClicked: Theme.reducedMotion = !Theme.reducedMotion
                        }
                    }
                }
                ControlGroup {
                    Layout.fillWidth: true
                    Text {
                        text: "Layout preview"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ControlChoice {
                        Layout.fillWidth: true
                        Accessible.name: "Preview zoom"
                        textRole: "name"
                        model: [
                            {
                                name: "100% preview zoom"
                            },
                            {
                                name: "125% preview zoom"
                            },
                            {
                                name: "150% preview zoom"
                            }
                        ]
                        onActivated: index => window.previewZoom = [1.0, 1.25, 1.5][index]
                    }
                    RowLayout {
                        Text {
                            text: "Right-to-left layout preview"
                            color: Material.colors.on_surface
                            font.family: Theme.typography.family
                            font.pixelSize: Material.type.bodyMedium
                            Layout.fillWidth: true
                        }
                        ControlSwitch {
                            text: "Right-to-left layout preview"
                            marked: window.mirroredPreview
                            onClicked: window.mirroredPreview = !window.mirroredPreview
                        }
                    }
                    WidgetStatus {
                        status: "loading"
                        message: "Zoom scales the preview; it does not change system text size."
                    }
                }
                ControlGroup {
                    id: actionsGroup
                    Layout.fillWidth: true
                    Text {
                        text: "Actions and state"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ControlAction {
                        Layout.fillWidth: true
                        text: "List action"
                        detail: "Supporting text"
                        symbol: "settings"
                        onClicked: window.activationCount++
                    }
                    ControlAction {
                        Layout.fillWidth: true
                        text: "Selected action"
                        detail: "Confirmed state"
                        symbol: "check"
                        marked: true
                        toggle: true
                    }
                    Text {
                        text: "Activations: " + window.activationCount
                        color: Material.colors.on_surface_variant
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                    Flow {
                        id: actionsFlow
                        Layout.fillWidth: true
                        spacing: 4
                        ControlAction {
                            text: "Compact"
                            compact: true
                        }
                        ControlAction {
                            text: "Disabled"
                            compact: true
                            enabled: false
                        }
                        ControlSwitch {
                            id: sampleSwitch
                            text: "Example switch"
                            onClicked: marked = !marked
                        }
                        ControlAction {
                            text: "Icon action"
                            symbol: "settings"
                            compact: true
                            iconOnly: true
                            Accessible.name: "Example icon action"
                            onClicked: window.activationCount++
                        }
                        ControlSwitch {
                            text: "Pending switch"
                            marked: true
                            pending: true
                            enabled: false
                        }
                    }
                }
                ControlGroup {
                    id: inputGroup
                    Layout.fillWidth: true
                    Text {
                        text: "Input"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    FoundationSlider {
                        id: sampleSlider
                        Layout.fillWidth: true
                        value: 40
                        Accessible.name: "Example slider"
                        onPreviewRequested: nextValue => window.sliderEvent = "Preview: " + Math.round(nextValue)
                        onCommitRequested: nextValue => window.sliderEvent = "Committed: " + Math.round(nextValue)
                    }
                    Text {
                        text: window.sliderEvent
                        color: Material.colors.on_surface_variant
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                    ControlChoice {
                        Layout.fillWidth: true
                        textRole: "name"
                        model: [
                            {
                                name: "First option"
                            },
                            {
                                name: "Second option"
                            }
                        ]
                    }
                    SearchInput {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 48
                        placeholderText: "Search example"
                        Accessible.name: "Search example"
                    }
                    ControlVolumeRow {
                        Layout.fillWidth: true
                        label: "Example application"
                        accessibleLabel: "Example application"
                        showMute: true
                        volume: window.sampleVolume
                        muted: window.sampleMuted
                        onVolumeRequested: value => window.sampleVolume = value
                        onMuteRequested: window.sampleMuted = !window.sampleMuted
                    }
                    WidgetStatus {
                        status: "error"
                        message: "Example error state"
                    }
                    WidgetStatus {
                        status: "unavailable"
                        message: "Device unavailable"
                    }
                }
                ControlGroup {
                    id: desktopGroup
                    Layout.fillWidth: true
                    Text {
                        text: "Desktop examples"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ApplicationRow {
                        Layout.fillWidth: true
                        entry: ({
                                name: "Example application",
                                genericName: "Launcher result",
                                icon: ""
                            })
                        selected: true
                    }
                    ClipboardRow {
                        Layout.fillWidth: true
                        entry: ({
                                label: "Example note",
                                detail: "Text",
                                kind: "text",
                                pinned: true
                            })
                        selected: false
                    }
                    ControlAction {
                        Layout.fillWidth: true
                        text: "Wireless headphones"
                        detail: "Connected · example device"
                        symbol: "headphones"
                        badge: "›"
                    }
                    ControlAction {
                        Layout.fillWidth: true
                        text: "Example Wi-Fi"
                        detail: "Connected · example network"
                        symbol: "wifi"
                        badge: "›"
                    }
                    ReplayControl {
                        id: exampleReplay
                        Layout.fillWidth: true
                        recording: recordingExample
                        onSetEnabled: enabled => {
                            recordingExample.state = enabled ? {
                                phase: "replay",
                                replay: true,
                                replay_status: "buffering"
                            } : {
                                phase: "idle",
                                replay: false,
                                replay_status: "off"
                            };
                        }
                        onSaveRequested: window.samplePlayhead = window.sampleStart
                    }
                    Text {
                        text: "Clip selection · sample frames · fixed 5:00 buffer"
                        color: Material.colors.on_surface_variant
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    ReplayTimeline {
                        Layout.fillWidth: true
                        duration: 300
                        startTime: window.sampleStart
                        endTime: window.sampleEnd
                        playhead: window.samplePlayhead
                        thumbnails: Array.from({
                            length: 10
                        }, (_, i) => ({
                                    url: "file://" + Quickshell.env("DF_FOUNDATION_ROOT") + "/wallpapers/wallhaven-135w7w.png",
                                    seconds: i * 30
                                }))
                        onSelectionRequested: (start, end) => {
                            window.sampleStart = Math.min(start, end - 0.05);
                            window.sampleEnd = end;
                        }
                        onScrubRequested: seconds => window.samplePlayhead = seconds
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        ControlTimeField {
                            Layout.fillWidth: true
                            label: "Clip start"
                            seconds: window.sampleStart
                            onRequested: seconds => window.sampleStart = Math.max(0, Math.min(seconds, window.sampleEnd - 0.05))
                        }
                        ControlTimeField {
                            Layout.fillWidth: true
                            label: "Clip end"
                            seconds: window.sampleEnd
                            onRequested: seconds => window.sampleEnd = Math.min(300, Math.max(seconds, window.sampleStart + 0.05))
                        }
                    }
                    Text {
                        text: (window.sampleEnd - window.sampleStart).toFixed(3) + " seconds selected"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                    WallpaperGrid {
                        Layout.fillWidth: true
                        backend: Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/component-gallery-fixture"
                        onFailed: message => console.warn("Gallery wallpaper example: " + message)
                    }
                    ControlAction {
                        id: openPopup
                        Layout.fillWidth: true
                        text: "Open popup example"
                        detail: "Escape or click outside to close"
                        onClicked: examplePopup.open()
                    }
                    WidgetStatus {
                        status: "loading"
                        message: "Loading example"
                    }
                    WidgetStatus {
                        status: "empty"
                        message: "Nothing here yet"
                    }
                }
            }
        }
        Popup {
            id: examplePopup
            parent: Overlay.overlay
            anchors.centerIn: parent
            width: Math.min(420, window.width - 32)
            height: 200
            padding: 0
            modal: true
            focus: true
            closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
            onOpened: popupClose.forceActiveFocus()
            onClosed: Material.restoreFocus(openPopup, Qt.PopupFocusReason)
            background: PopupCard {
                presented: examplePopup.opened
                dismissing: false
            }
            contentItem: ColumnLayout {
                anchors.fill: parent
                anchors.margins: 24
                Text {
                    text: "Popup example"
                    color: Material.colors.on_surface
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.titleMedium
                }
                Text {
                    Layout.fillWidth: true
                    text: "A local preview of the shared popup surface."
                    color: Material.colors.on_surface_variant
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.bodyMedium
                    wrapMode: Text.WordWrap
                }
                Item {
                    Layout.fillHeight: true
                }
                ControlAction {
                    id: popupClose
                    text: "Close"
                    compact: true
                    onClicked: examplePopup.close()
                }
            }
        }
        Timer {
            running: Quickshell.env("DF_GALLERY_TEST") === "1"
            interval: 100
            onTriggered: {
                const initial = sampleSwitch.marked;
                const oldMotion = Theme.reducedMotion;
                reducedMotionSwitch.clicked();
                const motionChanged = Theme.reducedMotion !== oldMotion;
                paletteChoice.activated(2);
                const lightSurface = String(Material.colors.surface);
                paletteChoice.activated(3);
                const darkSurface = String(Material.colors.surface);
                sampleSwitch.clicked();
                sampleSlider.value = 47;
                sampleSlider.moved();
                exampleReplay.setEnabled(true);
                examplePopup.open();
                const popupOpened = examplePopup.visible;
                examplePopup.close();
                const good = popupOpened && motionChanged && lightSurface !== darkSurface && sampleSwitch.marked !== initial && sampleSwitch.implicitHeight === Material.control.target && window.sliderEvent === "Committed: 47" && exampleReplay.statusText === "Buffering";
                if (!good) {
                    console.error("GALLERY_TEST_FAILED");
                    Qt.quit();
                    return;
                }
                const capture = Quickshell.env("DF_GALLERY_CAPTURE");
                if (capture) {
                    window.contentItem.grabToImage(result => {
                        if (result.saveToFile(capture))
                            console.log("GALLERY_TEST_OK");
                        else
                            console.error("GALLERY_TEST_FAILED");
                        Qt.quit();
                    });
                } else {
                    console.log("GALLERY_TEST_OK");
                    Qt.quit();
                }
            }
        }
    }
}
