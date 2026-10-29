import QtQuick
import QtQuick.Effects
import "../theme"

Rectangle {
    radius: Material.shape.extraLarge
    color: Material.colors.surface_container_low
    border.width: 0
    layer.enabled: true
    layer.effect: MultiEffect {
        shadowEnabled: true
        shadowColor: "#a0000000"
        shadowBlur: 0.7
        shadowVerticalOffset: 12
    }
}
