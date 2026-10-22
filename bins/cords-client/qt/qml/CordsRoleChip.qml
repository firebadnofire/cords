import QtQuick
import QtQuick.Layouts

Rectangle {
    id: chip
    property string roleName: ""
    property string roleColor: ""
    property real maximumWidth: 280
    property bool removable: false
    property bool removalEnabled: true
    signal removeRequested()
    readonly property string label: roleName.startsWith("$") ? roleName : "$" + roleName
    implicitWidth: Math.min(maximumWidth, content.implicitWidth + 16)
    implicitHeight: Math.max(24, content.implicitHeight + 8)
    color: Theme.input
    border.color: Theme.divider
    radius: Theme.radius
    Accessible.role: Accessible.StaticText
    Accessible.name: label
    RowLayout {
        id: content
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        anchors.topMargin: 4
        anchors.bottomMargin: 4
        spacing: 6
        Rectangle {
            Layout.preferredWidth: 8
            Layout.preferredHeight: 8
            radius: 4
            color: /^#[0-9a-fA-F]{6}$/.test(chip.roleColor) ? chip.roleColor : Theme.secondary
        }
        CordsLabel {
            Layout.fillWidth: true
            text: chip.label
            elide: Text.ElideRight
            wrapMode: Text.NoWrap
            font.pixelSize: Theme.captionSize
        }
        CordsButton {
            objectName: "removeRoleChip"
            visible: chip.removable
            enabled: chip.removalEnabled
            quiet: true
            text: "×"
            implicitWidth: 22
            implicitHeight: 22
            Accessible.name: "Remove " + chip.label
            onClicked: chip.removeRequested()
        }
    }
}
