pragma ComponentBehavior: Bound
import QtQuick
import "../theme"

// Same two-stage presentation in the desktop and gallery; no system operations.
Item {
    id: root
    property int selected: 0
    property int armed: -1
    readonly property var actions: [
        {
            id: "suspend",
            label: "Suspend",
            symbol: "dark_mode"
        },
        {
            id: "logout",
            label: "Log out",
            symbol: "logout"
        },
        {
            id: "reboot",
            label: "Reboot",
            symbol: "restart_alt"
        },
        {
            id: "poweroff",
            label: "Power off",
            symbol: "power_settings_new"
        }
    ]
    signal confirmed(string action)
    implicitHeight: armed >= 0 ? 200 : actions.length * 76
    function cancel(): void {
        armed = -1;
    }
    function navigate(delta: int): void {
        if (enabled && armed < 0)
            selected = Material.listIndex(selected + delta, actions.length);
    }
    function activate(index: int): void {
        if (!enabled || index < 0 || index >= actions.length)
            return;
        selected = index;
        if (armed === index)
            confirmed(actions[index].id);
        else
            armed = index;
    }
    Repeater {
        model: root.actions
        delegate: ControlButton {
            id: action
            required property var modelData
            required property int index
            objectName: "power-action-" + modelData.id
            x: 0
            y: root.armed >= 0 ? 20 : index * 76
            width: root.width
            height: root.armed >= 0 ? 160 : 72
            opacity: root.armed < 0 || root.armed === index ? 1 : 0
            visible: opacity > 0
            enabled: root.enabled && (root.armed < 0 || root.armed === index)
            Accessible.name: (root.armed === index ? "Confirm " : "") + modelData.label
            onClicked: root.activate(index)
            Behavior on y {
                SurfaceAnimation {}
            }
            Behavior on height {
                SurfaceAnimation {}
            }
            Behavior on opacity {
                SurfaceAnimation {}
            }
            background: Rectangle {
                radius: Material.shape.medium
                color: root.armed < 0 && root.selected === action.index ? Material.colors.secondary_container : "transparent"
                Behavior on color {
                    SurfaceColorAnimation {}
                }
                StateLayer {
                    color: Material.colors.primary
                    pressed: action.down
                    hovered: action.hovered
                    focused: action.visualFocus
                }
                FocusIndicator {
                    visible: action.visualFocus
                }
            }
            contentItem: Item {
                ControlIcon {
                    name: action.modelData.symbol
                    x: iconX.value
                    y: (parent.height - height) / 2
                    width: iconSize.value
                    height: width
                    color: Material.colors.primary
                    SpringValue {
                        id: iconSize
                        targetValue: root.armed === action.index ? 112 : 24
                    }
                    SpringValue {
                        id: iconX
                        targetValue: root.armed === action.index ? (action.availableWidth - 112) / 2 : 16
                    }
                }
                Text {
                    x: 56
                    width: parent.width - 72
                    anchors.verticalCenter: parent.verticalCenter
                    text: action.modelData.label
                    color: Material.colors.on_surface
                    font.family: Theme.typography.family
                    font.pixelSize: Material.type.bodyLarge
                    opacity: root.armed < 0 ? 1 : 0
                    Behavior on opacity {
                        SurfaceAnimation {}
                    }
                }
            }
        }
    }
}
