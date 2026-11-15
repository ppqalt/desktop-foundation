import QtQuick
import QtQuick.Layouts
import "../theme"

// Presentation only: production and gallery use this exact composition.
// Authentication and the compositor lock remain outside the component.
Item {
    id: root
    property string accountName: ""
    property string errorText: ""
    property string prompt: "Password"
    property bool busy: false
    property int clearEpoch: 0
    readonly property bool hasResponse: password.text.length > 0
    signal submitted()
    signal cancelled()
    function takeResponse(): string {
        const response = password.text;
        password.clear();
        return response;
    }
    function focusPassword(): void { password.forceActiveFocus(Qt.OtherFocusReason); }
    onClearEpochChanged: password.clear()
    implicitWidth: 360
    implicitHeight: body.implicitHeight
    ColumnLayout {
        id: body
        anchors.centerIn: parent
        width: Math.min(360, Math.max(1, root.width - 2 * Theme.spacing.large))
        spacing: Theme.spacing.medium
        Text {
            Layout.fillWidth: true
            text: root.accountName
            horizontalAlignment: Text.AlignHCenter
            color: Material.colors.on_surface
            font.family: Theme.typography.family
            font.pixelSize: Material.type.headlineSmall
            elide: Text.ElideRight
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: password.preferredFieldHeight + 16
            radius: Material.shape.small
            color: Material.colors.surface_container_highest
            border.color: password.activeFocus ? Material.colors.primary : Material.colors.outline_variant
            border.width: password.activeFocus ? 2 : 1
            SearchInput {
                id: password
                objectName: "lockPassword"
                anchors.fill: parent
                anchors.margins: 12
                placeholderText: root.prompt
                echoMode: TextInput.Password
                passwordMaskDelay: 0
                inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhHiddenText
                maximumLength: 1024
                enabled: !root.busy
                Accessible.name: root.prompt
                Keys.onReturnPressed: if (!root.busy && text.length > 0) root.submitted()
                Keys.onEnterPressed: if (!root.busy && text.length > 0) root.submitted()
                Keys.onEscapePressed: { clear(); root.cancelled(); }
            }
        }
        Text {
            Layout.fillWidth: true
            visible: root.errorText.length > 0
            text: root.errorText
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            color: Material.colors.error
            font.family: Theme.typography.family
            font.pixelSize: Material.type.bodyMedium
            Accessible.role: Accessible.AlertMessage
        }
        ControlAction {
            compact: true
            Layout.alignment: Qt.AlignHCenter
            text: "Unlock"
            enabled: !root.busy && root.hasResponse
            onClicked: root.submitted()
        }
    }
    Component.onCompleted: focusPassword()
}
