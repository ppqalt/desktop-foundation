import QtQuick
import "../theme"

// Presentation of an owner's loading/empty/unavailable/error state, never its authority.
Text {
    property string status: "error"
    property string message: ""
    text: message
    textFormat: Text.PlainText
    visible: status !== "ready" && message.length > 0
    color: status === "error" ? Material.colors.error : Material.colors.on_surface_variant
    font.family: Theme.typography.family
    font.pixelSize: Material.type.labelSmall
    wrapMode: Text.Wrap
    Accessible.role: Accessible.StaticText
    Accessible.name: message
}
