pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Wayland
import "../theme"

// qmllint disable uncreatable-type
PanelWindow {
    id: root
    property var targetScreen: null
    property int level: 0
    property bool muted: false
    property bool shown: false
    property bool awaitingFrame: false
    readonly property int feedbackDuration: 20
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
        radius: Theme.radii.medium
        color: Theme.colors.background
        border.width: 1
        border.color: Theme.colors.border
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
        Text {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.leftMargin: 14
            anchors.topMargin: 10
            text: root.muted ? "Muted" : "Volume"
            color: Theme.colors.muted
            font.family: Theme.typography.family
            font.pixelSize: 13
        }
        Text {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.rightMargin: 14
            anchors.topMargin: 10
            text: root.level + "%"
            color: Theme.colors.foreground
            font.family: Theme.typography.mono
            font.pixelSize: 13
        }
        Rectangle {
            id: track
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            anchors.bottomMargin: 12
            height: 2
            radius: 1
            color: Theme.colors.selected
            Rectangle {
                objectName: "volume-progress"
                height: parent.height
                width: Math.round(track.width * (root.muted ? 0 : root.level) / 100)
                radius: 1
                color: Theme.colors.accent
            }
        }
    }
}
