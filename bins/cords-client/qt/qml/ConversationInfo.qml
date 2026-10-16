pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ScrollView {
    id: info
    required property Controller desktop
    required property var channel
    required property string route
    contentWidth: availableWidth
    ColumnLayout {
        width: parent.width
        spacing: 12
        CordsLabel {
            text: "Conversation details"
            font.bold: true
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "Mode: " + (info.channel.mode || "No channel") + "\nEpoch: " + (info.channel.epoch || 0)
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "Server signing identity\n" + (info.desktop.state.server || "")
            Layout.fillWidth: true
        }
        CordsButton {
            text: "Copy server identity"
            onClicked: info.desktop.copy(info.desktop.state.server)
        }
        CordsLabel {
            text: "Channel ID\n" + info.route
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "A newly added device receives current and future keys. Earlier local history may differ between devices."
            color: Theme.secondary
            Layout.fillWidth: true
        }
    }
}
