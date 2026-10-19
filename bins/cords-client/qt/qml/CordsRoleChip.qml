import QtQuick
import QtQuick.Layouts

Rectangle {
    id: chip
    property string roleName: ""
    property string roleColor: ""
    readonly property string label: roleName.startsWith("$") ? roleName : "$" + roleName
    implicitWidth: content.implicitWidth + 16
    implicitHeight: Math.max(24, content.implicitHeight + 8)
    color: Theme.input
    border.color: Theme.divider
    radius: Theme.radius
    Accessible.role: Accessible.StaticText
    Accessible.name: label
    RowLayout {
        id: content
        anchors.centerIn: parent
        spacing: 6
        Rectangle {
            Layout.preferredWidth: 8
            Layout.preferredHeight: 8
            radius: 4
            color: /^#[0-9a-fA-F]{6}$/.test(chip.roleColor) ? chip.roleColor : Theme.secondary
        }
        CordsLabel {
            text: chip.label
            font.pixelSize: Theme.captionSize
        }
    }
}
