pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Wayland
import "../theme"
import "../components"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    property var targetScreen: null
    property int level: 0
    property bool muted: false
    property bool shown: false
    property bool awaitingFrame: false
    readonly property int feedbackDuration: 100
    screen: targetScreen
    visible: false
    implicitWidth: 260
    implicitHeight: 64
    anchors.top: true
    margins.top: 16
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "desktop-foundation-volume"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    color: "transparent"
    mask: Region {}
    function present(percent: int, isMuted: bool): void {
        if (!visible) {
            shown = false;
            awaitingFrame = true;
        }
        level = Math.max(0, Math.min(100, percent));
        muted = isMuted;
        visible = true;
        if (!awaitingFrame)
            shown = true;
        expiry.restart();
    }
    Timer {
        id: expiry
        interval: 1500
        onTriggered: {
            root.awaitingFrame = false;
            root.shown = false;
            if (card.opacity === 0)
                root.visible = false;
        }
    }
    Rectangle {
        id: card
        objectName: "volume-card"
        Connections {
            target: card.Window.window
            enabled: root.visible && root.awaitingFrame
            function onFrameSwapped(): void {
                root.awaitingFrame = false;
                root.shown = true;
            }
        }
        anchors.fill: parent
        anchors.margins: 3
        radius: Material.shape.extraLarge
        color: Material.colors.surface_container_low
        border.width: 0
        border.color: Material.colors.outline_variant
        opacity: root.shown ? 1 : 0
        onOpacityChanged: {
            if (opacity === 0 && !root.shown)
                root.visible = false;
        }
        Behavior on opacity {
            NumberAnimation {
                duration: root.feedbackDuration
                easing.type: Theme.easing
            }
        }
        ControlIcon {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.leftMargin: 14
            anchors.topMargin: 8
            name: root.muted ? "volume_off" : "volume_up"
            color: Material.colors.on_surface
        }
        Text {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.rightMargin: 14
            anchors.topMargin: 10
            text: root.level + "%"
            color: Material.colors.on_surface
            font.family: Theme.typography.family
            font.pixelSize: Material.type.labelLarge
            font.features: ({
                    "tnum": 1
                })
        }
        Rectangle {
            id: track
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            anchors.bottomMargin: 12
            height: 4
            radius: 2
            color: Material.colors.secondary_container
            Rectangle {
                objectName: "volume-progress"
                height: parent.height
                width: Math.round(track.width * (root.muted ? 0 : root.level) / 100)
                radius: 1
                color: Material.colors.primary
            }
        }
    }
}
