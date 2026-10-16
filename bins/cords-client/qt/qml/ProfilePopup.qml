pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CordsDialog {
    id: popup
    required property Controller desktop
    property string account: ""
    readonly property var profile: desktop.state.profile || ({})
    readonly property bool loaded: profile.account === account
    title: "Account profile"
    onOpened: if (account && !desktop.busy)
        desktop.loadProfile(account)
    ColumnLayout {
        width: parent.width
        spacing: 12
        CordsAvatar {
            id: profileAvatar
            name: popup.loaded ? popup.profile.name || popup.account : popup.account
            Layout.preferredWidth: 64
            Layout.preferredHeight: 64
            shape: popup.profile.shape || "circle"
            source: popup.loaded ? popup.profile.image || "" : ""
            playing: !Theme.reducedMotion && !(popup.desktop.state.preferences || {}).disableAnimations
        }
        CordsLabel {
            text: profileAvatar.imageError
            visible: text.length > 0
            color: Theme.warning
            Layout.fillWidth: true
        }
        CordsLabel {
            text: popup.loaded && popup.profile.name ? popup.profile.name : "Verified account"
            font.pixelSize: Theme.headingSize
            Layout.fillWidth: true
        }
        CordsField {
            text: popup.account
            readOnly: true
            selectByMouse: true
            Layout.fillWidth: true
            Accessible.name: "Account fingerprint"
        }
        CordsButton {
            text: "Copy fingerprint"
            onClicked: popup.desktop.copy(popup.account)
        }
        CordsLabel {
            visible: !popup.loaded
            text: popup.desktop.busy ? "Loading verified profile…" : "Profile unavailable. Try again after reconnecting."
            color: Theme.secondary
            Layout.fillWidth: true
        }
        CordsLabel {
            visible: !!popup.profile.pending
            text: popup.profile.pending || ""
            color: Theme.warning
            Layout.fillWidth: true
        }
        CordsLabel {
            visible: popup.loaded && (popup.profile.conflicts || []).length > 0
            text: "Conflicting signed profile changes need review: " + (popup.profile.conflicts || []).join(", ")
            color: Theme.warning
            Layout.fillWidth: true
        }
        CordsField {
            id: name
            visible: popup.account === popup.desktop.state.account && popup.loaded
            placeholderText: "New public display name"
            maximumLength: 100
            Layout.fillWidth: true
        }
        CordsButton {
            text: "Sign and save name"
            visible: name.visible
            enabled: !popup.desktop.busy && name.text.trim().length > 0 && (popup.profile.conflicts || []).length === 0
            onClicked: popup.desktop.saveProfileName(name.text, popup.profile.heads || [])
        }
        CordsLabel {
            text: popup.desktop.state.error || ""
            color: Theme.danger
            Layout.fillWidth: true
        }
        CordsButton {
            text: "Close"
            Layout.alignment: Qt.AlignRight
            onClicked: popup.close()
        }
    }
}
