import QtQuick
import Quickshell
import "../theme"

Rectangle {
    id: root
    required property var entry
    required property bool selected
    property string iconSource: Quickshell.iconPath(root.entry.icon, true) || "../assets/application.svg"
    property string iconText: ""
    property string iconName: ""
    property string actionLabel: "↵"
    // Setting indicators use reported state; the caller confirms successful readback.
    property var settingState: null
    property string settingValue: ""
    property bool pending: false
    property string pendingLabel: "Saving…"
    property bool confirmed: false
    property bool interactive: true
    readonly property bool hasSettingState: typeof settingState === "boolean"
    readonly property bool hasSetting: hasSettingState || settingValue !== ""
    Accessible.role: hasSettingState ? Accessible.CheckBox : Accessible.ListItem
    Accessible.name: entry.name + (settingValue ? ", " + settingValue : "")
    Accessible.checkable: hasSettingState
    Accessible.checked: hasSettingState && settingState
    Accessible.onPressAction: root.chosen()
    signal pointerMoved(real x, real y)
    signal hovered
    signal chosen
    implicitHeight: 72
    radius: Material.shape.medium
    color: selected ? Material.colors.secondary_container : (mouse.containsMouse ? Material.colors.surface_container_high : "transparent")
    border.width: selected || confirmed ? 1 : 0
    border.color: confirmed ? Material.colors.primary : Material.colors.outline
    Behavior on color {
        SurfaceColorAnimation {}
    }
    Behavior on border.color {
        SurfaceColorAnimation {}
    }
    Rectangle {
        width: 3
        height: parent.height - 24
        x: 1
        anchors.verticalCenter: parent.verticalCenter
        radius: 2
        color: Material.colors.primary
        opacity: root.confirmed ? 1 : 0
        Behavior on opacity {
            SurfaceAnimation {
                duration: Material.motion.fastEffects
            }
        }
    }
    StateLayer {
        color: Material.colors.primary
        pressed: mouse.pressed
    }
    Rectangle {
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: 38
        height: 38
        radius: 10
        color: Material.colors.surface_container_highest
        Image {
            id: icon
            anchors.centerIn: parent
            width: 26
            height: 26
            source: root.iconSource
            sourceSize.width: 32
            sourceSize.height: 32
            smooth: true
            visible: root.iconText === "" && root.iconName === "" && status !== Image.Error
        }
        Image {
            anchors.centerIn: parent
            width: 26
            height: 26
            source: "../assets/application.svg"
            visible: root.iconText === "" && root.iconName === "" && icon.status === Image.Error
        }
        ControlIcon {
            anchors.centerIn: parent
            name: root.iconName
            color: Material.colors.primary
            visible: root.iconName !== ""
        }
        Text {
            anchors.centerIn: parent
            visible: root.iconText !== ""
            text: root.iconText
            color: Material.colors.primary
            font.family: Theme.typography.family
            font.pixelSize: 24
        }
    }
    Column {
        x: 66
        width: Math.max(0, indicators.x - x - 12)
        anchors.verticalCenter: parent.verticalCenter
        spacing: 3
        Text {
            width: parent.width
            text: root.entry.name
            color: root.selected ? Material.colors.on_secondary_container : Material.colors.on_surface
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyLarge
            font.weight: Font.Normal
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            text: root.entry.genericName || root.entry.comment || ""
            visible: text !== ""
            color: Material.colors.on_surface_variant
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
            elide: Text.ElideRight
        }
    }
    Row {
        id: indicators
        anchors.right: parent.right
        anchors.rightMargin: 20
        anchors.verticalCenter: parent.verticalCenter
        spacing: 9
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.settingValue !== ""
            text: root.settingValue
            color: root.hasSettingState && !root.settingState ? Material.colors.on_surface_variant : Material.colors.primary
            font.family: Theme.typography.family
            font.pixelSize: 12
            font.weight: Font.Normal
            width: Math.min(implicitWidth, Math.max(40, root.width * 0.28))
            elide: Text.ElideRight
        }
        SwitchIndicator {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.hasSettingState
            marked: root.settingState === true
            pending: root.pending
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.pending ? root.pendingLabel : root.confirmed ? "✓" : root.actionLabel
            color: Material.colors.primary
            font.family: Theme.typography.family
            font.pixelSize: text === "↵" ? 20 : 11
            visible: root.pending || root.confirmed || (!root.hasSetting && (root.selected || root.actionLabel !== "↵"))
        }
    }
    HoverHandler {
        enabled: root.interactive
        onPointChanged: {
            if (hovered)
                root.pointerMoved(point.scenePosition.x, point.scenePosition.y);
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.interactive
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor
        onEntered: root.hovered()
        onClicked: root.chosen()
    }
}
