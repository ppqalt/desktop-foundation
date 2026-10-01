pragma ComponentBehavior: Bound
import QtQuick
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
        level = Math.max(0, Math.min(100, percent));
        muted = isMuted;
        visible = true;
        shown = true;
        expiry.restart();
    }
    Timer {
        id: expiry
        interval: 1500
        onTriggered: root.shown = false
    }
    Rectangle {
        id: card
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
                duration: Theme.timing.fast
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
                height: parent.height
                width: Math.round(track.width * (root.muted ? 0 : root.level) / 100)
                radius: 1
                color: Theme.colors.accent
                Behavior on width {
                    NumberAnimation {
                        duration: Theme.timing.fast
                        easing.type: Theme.easing
                    }
                }
            }
        }
    }
}
