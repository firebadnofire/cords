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
    property bool manualPlayback: false
    property bool playbackPaused: false
    objectName: "expandedProfile"
    title: "Account profile"
    width: Math.min(440, parent ? parent.width - 32 : 440)
    onOpened: {
        manualPlayback = false;
        playbackPaused = false;
        if (account && !desktop.busy)
            desktop.loadProfile(account);
    }
    onClosed: {
        manualPlayback = false;
        playbackPaused = false;
    }
    onAccountChanged: {
        manualPlayback = false;
        playbackPaused = false;
    }
    Connections {
        target: Theme
        function onReducedMotionChanged() {
            popup.manualPlayback = false;
        }
        function onDisableAnimationsChanged() {
            popup.manualPlayback = false;
        }
    }
    footer: Item {
        implicitHeight: 58
        CordsButton {
            text: "Close"
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 12
            onClicked: popup.close()
        }
    }
    contentHeight: Math.min(profileContent.implicitHeight, parent ? parent.height - 210 : 520)
    ScrollView {
        id: profileScroll
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth
        ColumnLayout {
            id: profileContent
            width: profileScroll.availableWidth
            spacing: 12
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 56
                color: Theme.elevated
                radius: Theme.radius
            }
            CordsAvatar {
                id: profileAvatar
                Layout.topMargin: -44
                Layout.leftMargin: 16
                account: popup.account
                name: popup.loaded ? popup.profile.name || popup.account : popup.account
                Layout.preferredWidth: 80
                Layout.preferredHeight: 80
                shape: popup.profile.shape || "circle"
                source: popup.loaded ? popup.profile.image || "" : ""
                playing: popup.opened && !popup.playbackPaused
                motionOverride: popup.manualPlayback
            }
            CordsButton {
                readonly property bool running: !popup.playbackPaused && (popup.manualPlayback || (!Theme.reducedMotion && !Theme.disableAnimations))
                text: running ? "Pause avatar" : "Play avatar"
                visible: popup.loaded && profileAvatar.animated
                enabled: profileAvatar.imageError.length === 0
                onClicked: {
                    popup.playbackPaused = running;
                    popup.manualPlayback = !running;
                }
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
        }
    }
}
