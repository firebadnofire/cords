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
    clip: true
    leftPadding: 16
    rightPadding: 16
    topPadding: 16
    ColumnLayout {
        width: info.availableWidth
        spacing: 12
        CordsAvatar {
            Layout.preferredWidth: 56
            Layout.preferredHeight: 56
            visible: !!info.desktop.state.serverProfile && !!info.desktop.state.serverProfile.hasIcon
            source: info.desktop.state.serverProfile && info.desktop.state.serverProfile.image || ""
            shape: info.desktop.state.serverProfile && info.desktop.state.serverProfile.shape || "circle"
            account: info.desktop.state.account || ""
            playing: info.visible
        }
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 64
            radius: Theme.radius
            visible: !!info.desktop.state.serverProfile && !!info.desktop.state.serverProfile.loaded
            color: info.desktop.state.serverProfile && info.desktop.state.serverProfile.background || "#314452"
        }
        CordsLabel {
            text: info.desktop.state.serverProfile && info.desktop.state.serverProfile.name || ""
            visible: !!text
            font.bold: true
            Layout.fillWidth: true
        }
        CordsLabel {
            text: info.desktop.state.serverProfile && info.desktop.state.serverProfile.description || ""
            visible: !!text
            color: Theme.secondary
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "Conversation details"
            font.bold: true
            Layout.fillWidth: true
        }
        CordsButton {
            text: info.desktop.state.routeMuted ? "Unmute conversation" : "Mute conversation"
            quiet: true
            enabled: !info.desktop.busy && !!info.route
            onClicked: info.desktop.muteRoute(info.route, !info.desktop.state.routeMuted)
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
