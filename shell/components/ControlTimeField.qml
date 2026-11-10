import QtQuick
import QtQuick.Layouts
import "../theme"

// A finite time edit. The owner validates source bounds and commits the value.
ColumnLayout {
    id: root
    property string label: "Time"
    property real seconds: 0
    property bool edited: false
    property alias input: field
    signal requested(real seconds)
    spacing: 2
    function format(value: real): string {
        const ms = Math.round(Math.max(0, value) * 100);
        return Math.floor(ms / 6000) + ":" + String(Math.floor(ms / 100) % 60).padStart(2, "0") + "." + String(ms % 100).padStart(2, "0");
    }
    Text {
        text: root.label
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.labelSmall
    }
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: field.preferredFieldHeight
        radius: Material.shape.small
        color: Material.colors.surface_container_highest
        border.width: field.activeFocus ? 2 : 0
        border.color: Material.colors.primary
        SearchInput {
            id: field
            anchors.fill: parent
            anchors.margins: 8
            Accessible.name: root.label
            text: root.format(root.seconds)
            placeholderText: "0:00.00"
            font.features: {
                "tnum": 1
            }
            validator: RegularExpressionValidator {
                regularExpression: /[0-9]{1,3}:[0-5][0-9](\.[0-9]{1,3})?/
            }
            onTextEdited: root.edited = true
            onEditingFinished: {
                if (acceptableInput && root.edited) {
                    const parts = text.split(":");
                    root.requested(Number(parts[0]) * 60 + Number(parts[1]));
                }
                root.edited = false;
                text = Qt.binding(() => root.format(root.seconds));
            }
        }
    }
}
