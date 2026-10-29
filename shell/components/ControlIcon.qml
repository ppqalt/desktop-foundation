import QtQuick
import "../theme"

Image {
    property string name: ""
    property color color: Material.colors.on_surface ?? "#e6e0e9"
    width: 24
    height: 24
    source: !Material.icons[name] ? "" : "data:image/svg+xml;utf8," + encodeURIComponent("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 -960 960 960' fill='" + color + "'><path d='" + (Material.icons[name] ?? "") + "'/></svg>")
    sourceSize.width: width * (Screen.devicePixelRatio || 1)
    sourceSize.height: height * (Screen.devicePixelRatio || 1)
    Accessible.ignored: true
}
