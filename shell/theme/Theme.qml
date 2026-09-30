pragma Singleton
import QtQuick

QtObject {
    // TEMPORARY test colors; no desktop visual design selected.
    readonly property var spacing: ({
            small: 4,
            medium: 8,
            large: 16
        })
    readonly property var radii: ({
            small: 2,
            medium: 4
        })
    readonly property var typography: ({
            family: "",
            small: 12,
            body: 14,
            heading: 18
        })
    readonly property var timing: ({
            fast: 100,
            normal: 150,
            compositor: 300
        })
    readonly property int easing: Easing.OutCubic
    readonly property var opacity: ({
            disabled: 0.5,
            background: 1.0
        })
    readonly property int blurRadius: 0
    readonly property var dimensions: ({
            probeWidth: 440,
            probeHeight: 100
        })
    readonly property var colors: ({
            background: "#202020",
            foreground: "#eeeeee",
            muted: "#aaaaaa",
            error: "#ff7777"
        })
    readonly property color testBackground: "#202020"
    readonly property color testForeground: "#eeeeee"
}
