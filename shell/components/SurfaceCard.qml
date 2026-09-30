import QtQuick
import QtQuick.Effects
import "../theme"

Rectangle {
    radius: Theme.radii.surface
    color: Theme.colors.background
    border.color: Theme.colors.border
    layer.enabled: true
    layer.effect: MultiEffect {
        shadowEnabled: true
        shadowColor: "#a0000000"
        shadowBlur: 0.7
        shadowVerticalOffset: 12
    }
}
