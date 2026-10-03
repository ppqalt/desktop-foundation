pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
    id: root
    property var generated: ({})
    property var paletteFile: FileView {
        path: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/desktop-foundation/theme/current/semantic.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                root.generated = JSON.parse(text());
            } catch (e) {}
        }
    }
    function reload(): void {
        paletteFile.reload();
    }
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
            family: "Google Sans",
            mono: "Google Sans Code",
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
    readonly property var fallback: ({
            background: "#ed171b22",
            elevated: "#222832",
            foreground: "#eef1f6",
            muted: "#939eae",
            subtle: "#616e80",
            border: "#38414e",
            accent: "#b8ceee",
            selected: "#293649",
            selectionBorder: "#485a73",
            iconTile: "#172029",
            hover: "#222b38",
            error: "#f1a5a5",
            scrim: "#50080b10"
        })
    readonly property var colors: {
        const mapped = Object.assign({}, fallback, generated);
        mapped.background = "#ed" + (generated.background ?? "#171b22").slice(1);
        return mapped;
    }
    readonly property color testBackground: "#202020"
    readonly property color testForeground: "#eeeeee"
}
