import QtQuick
import QtQuick.Layouts
import "../theme"

// One volume layout for device and application readback. Owners queue requests.
ColumnLayout {
    id: root
    property string label: ""
    property string accessibleLabel: label
    property string muteAccessibleLabel: accessibleLabel
    property string sliderName: ""
    property string muteName: ""
    property bool showMute: false
    property bool muted: false
    property string muteSymbol: "volume_up"
    property string mutedSymbol: "volume_off"
    property bool valueAvailable: true
    property real volume: 0
    property alias slider: slider
    property alias muteButton: mute
    signal volumeRequested(real value)
    signal muteRequested
    spacing: 0
    Text {
        visible: root.label !== ""
        Layout.fillWidth: true
        text: root.label
        textFormat: Text.PlainText
        elide: Text.ElideRight
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        ControlAction {
            id: mute
            objectName: root.muteName
            visible: root.showMute
            Layout.preferredWidth: Material.control.target
            text: root.muted ? "Unmute" : "Mute"
            Accessible.name: root.muteAccessibleLabel + " mute"
            symbol: root.muted ? root.mutedSymbol : root.muteSymbol
            compact: true
            toggle: true
            iconOnly: true
            marked: root.muted
            onClicked: root.muteRequested()
        }
        ControlVolume {
            id: slider
            objectName: root.sliderName
            Layout.fillWidth: true
            Accessible.name: root.accessibleLabel + " volume"
            Binding {
                target: slider
                property: "value"
                value: root.volume
                when: !slider.pressed
                restoreMode: Binding.RestoreNone
            }
            onPreviewRequested: value => root.volumeRequested(value)
        }
        Text {
            Layout.preferredWidth: 48
            horizontalAlignment: Text.AlignRight
            text: root.valueAvailable ? Math.round(slider.value) + "%" : "—"
            color: Material.colors.on_surface
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
            font.features: {
                "tnum": 1
            }
        }
    }
}
