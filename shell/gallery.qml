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
        property var previewPalettes: JSON.parse(paletteData.text())
        property FileView paletteData: FileView {
            path: Quickshell.env("DF_FOUNDATION_ROOT") + "/shell/gallery-palettes.json"
            blockLoading: true
        }
        function selectPalette(index: int): void {
            const keys = ["", "violet-dark", "violet-light", "teal-dark", "teal-light"];
            if (index === 0) {
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
            property int bufferSeconds: 135
            property var state: ({
                    phase: "idle",
                    replay: false,
                    replay_status: "off"
                })
        }
        Flickable {
            anchors.fill: parent
            contentWidth: width
            contentHeight: content.implicitHeight + 48
            clip: true
            ScrollBar.vertical: FoundationScrollbar {}
            ColumnLayout {
                id: content
                x: 24
                y: 24
                width: parent.width - 48
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
                        ControlSwitch {
                            text: "Pending switch"
                            marked: true
                            pending: true
                            enabled: false
                        }
                    }
                }
                ControlGroup {
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
                    WidgetStatus {
                        status: "error"
                        message: "Example error state"
                    }
                }
                ControlGroup {
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
                        onSaveRequested: recordingExample.state = {
                            phase: "replay",
                            replay: true,
                            replay_status: "buffering",
                            save_status: "saving"
                        }
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
                const good = lightSurface !== darkSurface && sampleSwitch.marked !== initial && sampleSwitch.implicitHeight === Material.control.target && window.sliderEvent === "Committed: 47" && exampleReplay.statusText === "Buffering";
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
