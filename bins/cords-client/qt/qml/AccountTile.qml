pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Button {
    id: tile
    property string account: ""
    property string label: ""
    property url avatarSource: ""
    property bool selected: false
    property bool addAccount: false
    property real centerX: 0.5
    property real centerY: 0.5
    property real zoom: 1
    implicitWidth: 156
    implicitHeight: 190
    padding: 8
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    Accessible.name: addAccount ? "Add new account" : label
    Accessible.description: selected ? "Selected identity" : "Select identity"
    background: Item {}
    contentItem: ColumnLayout {
        spacing: 14
        Item {
            Layout.preferredWidth: 120
            Layout.preferredHeight: 120
            Layout.alignment: Qt.AlignHCenter
            scale: tile.hovered || tile.visualFocus || tile.selected ? 1.05 : 1
            Behavior on scale {
                NumberAnimation {
                    duration: Theme.reducedMotion ? 0 : 140
                }
            }
            Rectangle {
                anchors.fill: parent
                anchors.margins: -7
                radius: width / 2
                color: "transparent"
                border.width: tile.selected ? 3 : tile.visualFocus || tile.hovered ? 2 : 0
                border.color: tile.selected || tile.visualFocus ? Theme.focus : Theme.accent
            }
            CordsAvatar {
                anchors.fill: parent
                visible: !tile.addAccount
                account: tile.account
                name: tile.label
                source: tile.avatarSource
                shape: "circle"
                centerX: tile.centerX
                centerY: tile.centerY
                zoom: tile.zoom
                playing: false
            }
            Rectangle {
                anchors.fill: parent
                visible: tile.addAccount
                radius: width / 2
                color: Theme.elevated
                Text {
                    anchors.centerIn: parent
                    text: "+"
                    font.pixelSize: 52
                    font.weight: Font.Light
                    color: Theme.text
                }
            }
        }
        CordsLabel {
            text: tile.addAccount ? "Add new" : tile.label
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            maximumLineCount: 2
            elide: Text.ElideRight
            font.bold: tile.selected
        }
        Item {
            Layout.fillHeight: true
        }
    }
}
