import QtQuick
import QtQuick.Controls
import "../theme"

AbstractButton {
    id: root
    property string detail: ""
    property string badge: ""
    property string symbol: ""
    property bool marked: false
    property bool compact: false
    property bool toggle: false
    property bool iconOnly: false
    // Compact = M3 small button, 40dp visual in a 48dp input target.
    // Otherwise use M3 one/two-line list items, 56/72dp.
    implicitHeight: compact ? 48 : detail ? 72 : 56
    implicitWidth: compact ? iconOnly ? 48 : 128 : 240
    activeFocusOnTab: true
    Accessible.name: text + (detail ? ", " + detail : "") + (badge ? ", " + badge : "")
    Accessible.role: toggle ? Accessible.CheckBox : Accessible.Button
    Accessible.checkable: toggle
    Accessible.checked: toggle && marked
    Keys.onReturnPressed: event => {
        if (!event.isAutoRepeat)
            root.clicked();
        event.accepted = true;
    }
    Keys.onEnterPressed: event => {
        if (!event.isAutoRepeat)
            root.clicked();
        event.accepted = true;
    }
    Keys.onPressed: event => {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
            return;
        const direction = Material.direction(event.key);
        if (direction) {
            Material.moveFocus(root, direction[0], direction[1]);
            event.accepted = true;
        }
    }
    readonly property color ink: !enabled ? Material.colors.on_surface : compact ? marked ? Material.colors.on_primary : toggle ? Material.colors.on_surface_variant : Material.colors.on_secondary_container : marked ? Material.colors.on_secondary_container : Material.colors.on_surface
    background: Item {
        Rectangle {
            id: fill
            anchors.fill: parent
            anchors.topMargin: root.compact ? 4 : 0
            anchors.bottomMargin: root.compact ? 4 : 0
            radius: root.compact ? root.down ? 8 : root.toggle && root.marked ? 12 : height / 2 : 12
            color: !root.enabled ? Material.colors.on_surface : root.compact ? root.marked ? Material.colors.primary : root.toggle ? Material.colors.surface_container : Material.colors.secondary_container : root.marked ? Material.colors.secondary_container : "transparent"
            opacity: root.enabled ? 1 : 0.12
            Behavior on radius {
                SurfaceAnimation {
                    duration: Material.motion.fastSpatial
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: Material.spatialCurve
                }
            }
            Rectangle {
                anchors.fill: parent
                radius: parent.radius
                color: root.ink
                opacity: !root.enabled ? 0 : root.down ? 0.10 : root.activeFocus ? 0.10 : root.hovered ? 0.08 : 0
                Behavior on opacity {
                    SurfaceAnimation {
                        duration: Material.motion.fastEffects
                        easing.type: Easing.BezierSpline
                        easing.bezierCurve: Material.effectsCurve
                    }
                }
            }
        }
        FocusIndicator {
            anchors.fill: fill
            cornerRadius: fill.radius
            visible: root.activeFocus
        }
    }
    contentItem: Item {
        opacity: root.enabled ? 1 : 0.38
        ControlIcon {
            id: glyph
            anchors.left: parent.left
            anchors.leftMargin: root.iconOnly ? (parent.width - width) / 2 : 16
            anchors.verticalCenter: parent.verticalCenter
            name: root.symbol
            width: root.compact && !root.iconOnly ? 20 : 24
            height: width
            color: root.ink
            visible: root.symbol !== ""
        }
        Column {
            visible: !root.iconOnly
            anchors.left: parent.left
            anchors.leftMargin: root.symbol ? root.compact ? 44 : 56 : 16
            anchors.right: indicator.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            Text {
                width: parent.width
                text: root.text
                textFormat: Text.PlainText
                color: root.ink
                elide: Text.ElideRight
                font.family: Theme.typography.family
                font.pixelSize: root.compact ? Material.type.labelLarge : Material.type.bodyLarge
                font.weight: root.compact ? Font.Medium : Font.Normal
                font.letterSpacing: root.compact ? 0.1 : 0.5
                lineHeightMode: Text.FixedHeight
                lineHeight: root.compact ? Material.type.labelLine : Material.type.bodyLargeLine
            }
            Text {
                width: parent.width
                visible: root.detail !== ""
                text: root.detail
                textFormat: Text.PlainText
                color: Material.colors.on_surface_variant
                elide: Text.ElideRight
                font.family: Theme.typography.family
                font.pixelSize: Material.type.bodyMedium
                font.letterSpacing: 0.25
                lineHeightMode: Text.FixedHeight
                lineHeight: Material.type.bodyMediumLine
            }
        }
        Text {
            id: indicator
            anchors.right: parent.right
            anchors.rightMargin: 16
            anchors.verticalCenter: parent.verticalCenter
            text: root.badge
            textFormat: Text.PlainText
            color: root.ink
            font.family: Theme.typography.family
            font.pixelSize: Material.type.labelLarge
        }
    }
}
