import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Quickshell
import "theme"
import "components"

// Developer preview of production components. This has no system backends.
ShellRoot {
    Window {
        id: window
        width: 760
        height: 640
        visible: true
        title: "Foundation component gallery"
        color: Material.colors.surface
        Flickable {
            anchors.fill: parent
            contentWidth: width
            contentHeight: content.implicitHeight + 48
            clip: true
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
                    }
                    RowLayout {
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
            }
        }
        Timer {
            running: Quickshell.env("DF_GALLERY_TEST") === "1"
            interval: 100
            onTriggered: {
                if (sampleSwitch.implicitHeight !== Material.control.target || sampleSlider.value !== 40)
                    console.error("GALLERY_TEST_FAILED");
                else
                    console.log("GALLERY_TEST_OK");
                Qt.quit();
            }
        }
    }
}
