pragma Singleton
import QtQuick

QtObject {
    // Shared graphite/glass surface language for launcher and clipboard.
    readonly property var spacing: ({
            small: 6,
            medium: 12,
            large: 24
        })
    readonly property var radii: ({
            small: 8,
            medium: 12,
            surface: 24
        })
    readonly property var typography: ({
            family: "Adwaita Sans",
            mono: "Adwaita Mono",
            small: 11,
            body: 14,
            heading: 22
        })
    readonly property var timing: ({
            fast: 100,
            normal: 160,
            exit: 120,
            compositor: 300
        })
    readonly property int easing: Easing.OutCubic
    readonly property var opacity: ({
            disabled: 0.45,
            background: 0.94
        })
    readonly property var dimensions: ({
            probeWidth: 440,
            probeHeight: 100,
            launcherWidth: 640,
            rowHeight: 62
        })
    readonly property var colors: ({
            background: "#ed171b22",
            elevated: "#222832",
            foreground: "#eef1f6",
            muted: "#939eae",
            subtle: "#616e80",
            border: "#38414e",
            accent: "#b8ceee",
            selected: "#293649",
            hover: "#222b38",
            error: "#f1a5a5",
            scrim: "#50080b10"
        })
    readonly property color testBackground: "#202020"
    readonly property color testForeground: "#eeeeee"
}
