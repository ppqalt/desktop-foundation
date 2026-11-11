import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "theme"
import "components"
import "surfaces"

// Developer preview of production components. This has no system backends.
ShellRoot {
    ReloadNotice {}
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
        property bool inputsDisabled: false
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
        FocusViewport {
            viewport: galleryScroll
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
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: "Foundation components"
                    color: Material.colors.on_surface
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.headlineSmall
                }
                Text {
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: "Interactive examples use local sample state."
                    color: Material.colors.on_surface_variant
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.bodyMedium
                }
                ControlChoice {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
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
                        },
                        {
                            name: "Motion"
                        }
                    ]
                    onActivated: index => {
                        const items = [null, actionsGroup, inputGroup, desktopGroup, motionGroup];
                        if (items[index])
                            galleryScroll.contentY = Math.min(galleryScroll.contentHeight - galleryScroll.height, Math.max(0, items[index].y * window.previewZoom));
                    }
                }
                ControlGroup {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Preview settings"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ControlChoice {
                        id: paletteChoice
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
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
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Text {
                            wrapMode: Text.WordWrap
                            text: "Reduced motion (preview only)"
                            color: Material.colors.on_surface
                            font.family: Theme.typography.family
                            font.pixelSize: Material.type.bodyMedium
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
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
                    Layout.minimumWidth: 0
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Layout preview"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ControlChoice {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
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
                    ControlChoice {
                        id: textScaleChoice
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Accessible.name: "Text size preview"
                        textRole: "name"
                        model: [
                            {
                                name: "100% text size"
                            },
                            {
                                name: "125% text size"
                            },
                            {
                                name: "150% text size"
                            },
                            {
                                name: "200% text size"
                            }
                        ]
                        onActivated: index => Material.textScale = [1.0, 1.25, 1.5, 2.0][index]
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Text {
                            wrapMode: Text.WordWrap
                            text: "Right-to-left layout preview"
                            color: Material.colors.on_surface
                            font.family: Theme.typography.family
                            font.pixelSize: Material.type.bodyMedium
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                        }
                        ControlSwitch {
                            text: "Right-to-left layout preview"
                            marked: window.mirroredPreview
                            onClicked: window.mirroredPreview = !window.mirroredPreview
                        }
                    }
                    WidgetStatus {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        status: "loading"
                        message: "Text size changes shared typography independently of preview zoom; neither changes system settings."
                    }
                }
                ControlGroup {
                    id: actionsGroup
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Actions and state"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ControlAction {
                        id: sampleListAction
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "List action"
                        detail: "Supporting text"
                        symbol: "settings"
                        onClicked: window.activationCount++
                    }
                    ControlAction {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Selected action"
                        detail: "Confirmed state"
                        onClicked: marked = !marked
                        symbol: "check"
                        marked: true
                        toggle: true
                    }
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Activations: " + window.activationCount
                        color: Material.colors.on_surface_variant
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                    Flow {
                        id: actionsFlow
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        spacing: 4
                        ControlAction {
                            text: "Compact"
                            compact: true
                            onClicked: window.activationCount++
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
                    Layout.minimumWidth: 0
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Input"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ControlAction {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Disable input examples"
                        compact: true
                        toggle: true
                        marked: window.inputsDisabled
                        onClicked: window.inputsDisabled = !window.inputsDisabled
                    }
                    FoundationSlider {
                        id: sampleSlider
                        enabled: !window.inputsDisabled
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        value: 40
                        Accessible.name: "Example slider"
                        onPreviewRequested: nextValue => window.sliderEvent = "Preview: " + Math.round(nextValue)
                        onCommitRequested: nextValue => window.sliderEvent = "Committed: " + Math.round(nextValue)
                    }
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: window.sliderEvent
                        color: Material.colors.on_surface_variant
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                    FoundationRangeSlider {
                        id: sampleRange
                        enabled: !window.inputsDisabled
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        first.value: 20
                        second.value: 80
                        startLabel: "Example range start"
                        endLabel: "Example range end"
                        onPreviewRequested: (start, end) => window.sliderEvent = "Preview: " + Math.round(start) + "–" + Math.round(end)
                        onCommitRequested: (start, end) => window.sliderEvent = "Committed: " + Math.round(start) + "–" + Math.round(end)
                    }
                    FoundationSlider {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        value: 40
                        enabled: false
                        Accessible.name: "Disabled example slider"
                    }
                    ControlChoice {
                        id: sampleChoice
                        enabled: !window.inputsDisabled
                        Accessible.name: "Example choice"
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        textRole: "name"
                        model: [
                            {
                                name: "First option"
                            },
                            {
                                name: "Second option"
                            },
                            {
                                name: "A longer option for checking narrow layouts and text scaling"
                            }
                        ]
                    }
                    ControlChoice {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Accessible.name: "Empty choice example"
                        enabled: !window.inputsDisabled
                        textRole: "name"
                        model: []
                    }
                    SearchInput {
                        id: sampleSearch
                        enabled: !window.inputsDisabled
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Layout.preferredHeight: preferredFieldHeight
                        placeholderText: "Search example"
                        Accessible.name: "Search example"
                    }
                    ControlVolumeRow {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        enabled: !window.inputsDisabled
                        label: "Example application"
                        accessibleLabel: "Example application"
                        showMute: true
                        volume: window.sampleVolume
                        muted: window.sampleMuted
                        onVolumeRequested: value => window.sampleVolume = value
                        onMuteRequested: window.sampleMuted = !window.sampleMuted
                    }
                    WidgetStatus {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        status: "error"
                        message: "Example error state"
                    }
                    WidgetStatus {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        status: "unavailable"
                        message: "Device unavailable"
                    }
                }
                ControlGroup {
                    id: desktopGroup
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Desktop examples"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.titleMedium
                    }
                    ApplicationRow {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        entry: ({
                                name: "Example application",
                                genericName: "Launcher result",
                                icon: ""
                            })
                        selected: true
                    }
                    ClipboardRow {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
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
                        Layout.minimumWidth: 0
                        text: "Wireless headphones"
                        detail: "Connected · example device"
                        symbol: "headphones"
                        badge: "›"
                    }
                    ControlAction {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Example Wi-Fi"
                        detail: "Connected · example network"
                        symbol: "wifi"
                        badge: "›"
                    }
                    RecordingControl {
                        id: exampleCapture
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        recording: recordingExample
                        onRequested: requestArgs => {
                            const stop = requestArgs[0] === "stop-recording";
                            recordingExample.state = stop ? {
                                phase: "idle",
                                replay: false,
                                replay_status: "off"
                            } : {
                                phase: "recording",
                                control_protocol: 2,
                                replay: false,
                                replay_status: "off",
                                audio_mode: requestArgs.includes("--no-audio") ? "none" : requestArgs.includes("--desktop-only") ? "desktop" : "desktop-microphone"
                            };
                        }
                    }
                    ReplayControl {
                        id: exampleReplay
                        showHeading: false
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        recording: recordingExample
                        onSetEnabled: enabled => {
                            recordingExample.state = enabled ? {
                                phase: "replay",
                                control_protocol: 2,
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
                        Layout.minimumWidth: 0
                    }
                    ReplayTimeline {
                        id: galleryTimeline
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
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
                        Layout.minimumWidth: 0
                        ControlTimeField {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            label: "Clip start"
                            seconds: window.sampleStart
                            onRequested: seconds => window.sampleStart = Math.max(0, Math.min(seconds, window.sampleEnd - 0.05))
                        }
                        ControlTimeField {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            label: "Clip end"
                            seconds: window.sampleEnd
                            onRequested: seconds => window.sampleEnd = Math.min(300, Math.max(seconds, window.sampleStart + 0.05))
                        }
                    }
                    Text {
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: (window.sampleEnd - window.sampleStart).toFixed(2) + " s selected"
                        color: Material.colors.on_surface
                        font.family: Theme.typography.family
                        font.pixelSize: Material.type.bodyMedium
                    }
                    WallpaperGrid {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        backend: Quickshell.env("DF_FOUNDATION_ROOT") + "/scripts/component-gallery-fixture"
                        onFailed: message => console.warn("Gallery wallpaper example: " + message)
                    }
                    ControlAction {
                        id: openPopup
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: "Open popup example"
                        detail: "Escape or click outside to close"
                        onClicked: examplePopup.open()
                    }
                    WidgetStatus {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        status: "loading"
                        message: "Loading example"
                    }
                    WidgetStatus {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        status: "empty"
                        message: "Nothing here yet"
                    }
                }
                MotionGallery {
                    id: motionGroup
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
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
            property bool keyboardOpened: false
            onAboutToShow: keyboardOpened = openPopup.visualFocus
            onOpened: Material.restoreFocus(popupClose, keyboardOpened ? Qt.TabFocusReason : Qt.MouseFocusReason)
            onClosed: Material.restoreFocus(openPopup, keyboardOpened ? Qt.TabFocusReason : Qt.MouseFocusReason)
            background: PopupCard {
                presented: examplePopup.opened
                dismissing: false
            }
            contentItem: ColumnLayout {
                anchors.fill: parent
                anchors.margins: 24
                Text {
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: "Popup example"
                    color: Material.colors.on_surface
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.titleMedium
                }
                Text {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
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
                window.inputsDisabled = true;
                const disabledInputs = !sampleSlider.enabled && !sampleRange.enabled && !sampleChoice.enabled && !sampleSearch.enabled;
                window.inputsDisabled = false;
                const initial = sampleSwitch.marked;
                const normalText = Material.type.bodyLarge;
                textScaleChoice.activated(3);
                const textScaled = Material.type.bodyLarge === normalText * 2 && sampleListAction.implicitHeight >= Material.type.bodyLargeLine + Material.type.bodyMediumLine + 24;
                textScaleChoice.activated(0);
                const oldMotion = Theme.reducedMotion;
                reducedMotionSwitch.clicked();
                const motionChanged = Theme.reducedMotion !== oldMotion;
                paletteChoice.activated(2);
                const lightSurface = String(Material.colors.surface);
                paletteChoice.activated(3);
                const darkSurface = String(Material.colors.surface);
                sampleSwitch.clicked();
                sampleSlider.value = 47;
                sampleRange.first.value = 25;
                sampleRange.first.moved();
                const rangeCommitted = window.sliderEvent === "Committed: 25–80";
                sampleSlider.moved();
                exampleCapture.modeChoice.activated(2);
                exampleCapture.actionButton.clicked();
                const captureStarted = recordingExample.state.audio_mode === "none" && exampleCapture.actionButton.enabled && exampleCapture.actionButton.text === "Stop";
                exampleCapture.actionButton.clicked();
                const captureStopped = recordingExample.state.phase === "idle";
                exampleReplay.setEnabled(true);
                examplePopup.open();
                const popupOpened = examplePopup.visible && Math.abs(examplePopup.background.width - examplePopup.width) <= 1 && Math.abs(examplePopup.background.height - examplePopup.height) <= 1 && examplePopup.background.border.width > 0;
                examplePopup.close();
                const good = exampleCapture.modeChoice.flat && captureStarted && captureStopped && disabledInputs && textScaled && rangeCommitted && popupOpened && motionChanged && lightSurface !== darkSurface && sampleSwitch.marked !== initial && sampleSwitch.implicitHeight === Material.control.target && window.sliderEvent === "Committed: 47" && exampleReplay.statusText === "Buffering";
                if (!good) {
                    console.error("GALLERY_TEST_FAILED");
                    Qt.quit();
                    return;
                }
                const captureScale = Number(Quickshell.env("DF_GALLERY_TEXT_SCALE")) || 1;
                textScaleChoice.currentIndex = [1, 1.25, 1.5, 2].indexOf(captureScale);
                Material.textScale = captureScale;
                const capture = Quickshell.env("DF_GALLERY_CAPTURE");
                if (capture) {
                    captureTimer.start();
                } else {
                    console.log("GALLERY_TEST_OK");
                    Qt.quit();
                }
            }
        }
        Timer {
            id: captureTimer
            interval: 100
            onTriggered: {
                const capture = Quickshell.env("DF_GALLERY_CAPTURE");
                const section = Quickshell.env("DF_GALLERY_CAPTURE_SECTION");
                if (section === "popup")
                    examplePopup.open();
                const target = section === "recording" ? exampleCapture : section === "motion" ? motionGroup : section === "input" ? inputGroup : section === "actions" ? actionsGroup : section === "desktop" ? desktopGroup : section === "replay" ? galleryTimeline : section === "choice" ? sampleChoice : null;
                if (target)
                    galleryScroll.contentY = Math.max(0, Math.min(galleryScroll.contentHeight - galleryScroll.height, target.mapToItem(content, 0, 0).y * window.previewZoom));
                if (section === "choice")
                    sampleChoice.popup.open();
                if (section === "motion")
                    motionGroup.showPreview();
                captureDelay.capturePath = capture;
                captureDelay.start();
            }
        }
        Timer {
            id: captureDelay
            property string capturePath: ""
            interval: 700
            onTriggered: {
                window.contentItem.grabToImage(result => {
                    if (result.saveToFile(captureDelay.capturePath))
                        console.log("GALLERY_TEST_OK");
                    else
                        console.error("GALLERY_TEST_FAILED");
                    Qt.quit();
                });
            }
        }
    }
}
