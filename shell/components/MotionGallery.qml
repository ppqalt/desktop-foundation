import QtQuick
import QtQuick.Layouts
import "../theme"

// Production primitives, not copies of their animation curves or lifecycles.
ControlGroup {
    id: root
    property bool sidebarOpen: false
    property int sidebarCompletions: 0
    property int popupCompletions: 0
    function showPreview(): void {
        sidebarOpen = true;
        sidebarMotion.move(true);
        popupPreview.visible = true;
        popupLifecycle.present();
    }
    Text {
        Layout.fillWidth: true
        text: "Motion"
        color: Material.colors.on_surface
        font.family: Theme.typography.family
        font.pixelSize: Material.type.titleMedium
    }
    Text {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        text: "Try rapid reversals, press and hold, or enable reduced motion above. These are the desktop's shared components."
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.bodyMedium
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        ControlAction {
            compact: true
            objectName: "motion-sidebar-toggle"
            text: "Sidebar"
            onClicked: {
                root.sidebarOpen = !root.sidebarOpen;
                sidebarMotion.move(root.sidebarOpen);
            }
        }
        ControlAction {
            compact: true
            objectName: "motion-popup-toggle"
            text: popupLifecycle.closing || !popupPreview.visible ? "Open popup" : "Close popup"
            onClicked: {
                if (popupLifecycle.closing || !popupPreview.visible) {
                    popupPreview.visible = true;
                    popupLifecycle.present();
                } else {
                    popupLifecycle.dismiss();
                }
            }
        }
        ControlSwitch {
            text: "Animated switch"
            onClicked: marked = !marked
        }
    }
    Item {
        id: viewport
        Layout.fillWidth: true
        implicitHeight: 176
        clip: true
        SurfaceCard {
            width: Math.min(128, viewport.width * 0.4)
            height: parent.height - 8
            y: 4
            x: -width * (1 - sidebarMotion.position)
            Text {
                anchors.centerIn: parent
                objectName: "motion-sidebar-toggle"
                text: "Sidebar"
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
            }
        }
        PopupCard {
            id: popupPreview
            visible: false
            width: Math.min(240, viewport.width - 16)
            height: 112
            presented: popupLifecycle.entered
            dismissing: popupLifecycle.closing
            Text {
                anchors.centerIn: parent
                text: "Popup"
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
            }
        }
    }
    SidebarMotion {
        id: sidebarMotion
        targetItem: viewport
        onSettled: root.sidebarCompletions++
    }
    PopupLifecycle {
        id: popupLifecycle
        targetItem: popupPreview
        onFinished: {
            root.popupCompletions++;
            popupPreview.visible = false;
        }
    }
    Text {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        text: "Sidebar: " + (sidebarMotion.running ? "moving" : root.sidebarOpen ? "open" : "closed") + " · Popup: " + (popupPreview.visible ? popupLifecycle.phase : "hidden")
        color: Material.colors.on_surface_variant
        font.family: Theme.typography.family
        font.pixelSize: Material.type.labelSmall
    }
}
