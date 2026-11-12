import QtQuick
import QtQuick.Layouts
import "../theme"

// Production primitives, not copies of their animation curves or lifecycles.
ControlGroup {
    id: root
    property bool sidebarOpen: false
    property int sidebarCompletions: 0
    property int popupCompletions: 0
    property bool detailsOpen: false
    property int pageFocusReason: Qt.OtherFocusReason
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
        ControlAction {
            id: detailsToggle
            compact: true
            objectName: "motion-details-toggle"
            text: root.detailsOpen ? "Hide details" : "Details"
            onClicked: root.detailsOpen = !root.detailsOpen
        }
    }
    ContentReveal {
        id: details
        objectName: "motion-details-reveal"
        Layout.fillWidth: true
        expanded: root.detailsOpen
        ControlGroup {
            width: parent.width
            Text {
                text: "Details"
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.titleMedium
            }
            ControlAction {
                Layout.fillWidth: true
                text: "Close"
                onClicked: {
                    const reason = visualFocus ? Qt.BacktabFocusReason : Qt.MouseFocusReason;
                    root.detailsOpen = false;
                    Material.restoreFocus(detailsToggle, reason);
                }
            }
        }
    }
    PagePane {
        id: pageMotion
        objectName: "motion-page-transition"
        Layout.fillWidth: true
        implicitHeight: pageExample.implicitHeight
        onCommitted: Material.restoreFocus(pageToggle, root.pageFocusReason)
        ControlGroup {
            id: pageExample
            anchors.fill: parent
            Text {
                text: pageMotion.currentKey === "overview" ? "Overview" : "Audio"
                color: Material.colors.on_surface
                font.family: Theme.typography.family
                font.pixelSize: Material.type.titleMedium
            }
            ControlAction {
                id: pageToggle
                compact: true
                objectName: "motion-page-toggle"
                text: pageMotion.currentKey === "overview" ? "Open Audio" : "Back"
                onClicked: {
                    root.pageFocusReason = visualFocus ? Qt.TabFocusReason : Qt.MouseFocusReason;
                    pageMotion.request(pageMotion.currentKey === "overview" ? "audio" : "overview", pageMotion.currentKey === "overview");
                }
            }
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
