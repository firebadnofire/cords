pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ScrollView {
    id: page
    required property Controller desktop
    signal profileRequested
    contentWidth: availableWidth
    property var draft: ({})
    property bool dirty: false
    property bool saving: false
    Connections {
        target: page.desktop
        function onStateChanged() {
            if (page.saving && !page.desktop.busy) {
                page.saving = false;
                if (page.desktop.state.notice === "Preferences saved" && !page.desktop.state.error)
                    page.reload();
            }
        }
    }
    function reload() {
        draft = Object.assign({}, desktop.state.preferences || ({}));
        dirty = false;
    }
    function change(key, value) {
        let next = Object.assign({}, draft);
        next[key] = value;
        draft = next;
        dirty = true;
    }
    onVisibleChanged: if (visible && !dirty)
        reload()
    Component.onCompleted: reload()
    ColumnLayout {
        width: Math.max(240, page.availableWidth - 32)
        x: 16
        spacing: 14
        CordsLabel {
            text: "Your account"
            font.pixelSize: Theme.headingSize
            Layout.topMargin: 16
        }
        CordsField {
            text: page.desktop.state.account || ""
            readOnly: true
            Layout.fillWidth: true
            Accessible.name: "Account fingerprint"
        }
        CordsButton {
            text: "Open canonical public profile"
            enabled: !page.desktop.busy
            onClicked: page.profileRequested()
        }
        CordsLabel {
            text: "Local nickname"
            font.bold: true
        }
        CordsField {
            text: page.draft.name || ""
            placeholderText: "Local account nickname"
            maximumLength: 100
            Layout.fillWidth: true
            onTextEdited: page.change("name", text)
        }
        CordsLabel {
            text: "Appearance"
            font.pixelSize: Theme.headingSize
        }
        RowLayout {
            Repeater {
                model: ["dark", "light", "midnight"]
                delegate: CordsButton {
                    required property string modelData
                    text: modelData
                    primary: page.draft.theme === modelData
                    onClicked: page.change("theme", modelData)
                }
            }
        }
        CordsCheck {
            text: "Compact spacing"
            checked: !!page.draft.compact
            onToggled: page.change("compact", checked)
        }
        CordsCheck {
            text: "Show public member panel"
            checked: !!page.draft.showMembers
            onToggled: page.change("showMembers", checked)
        }
        CordsCheck {
            text: "Reduced motion"
            checked: !!page.draft.reducedMotion
            onToggled: page.change("reducedMotion", checked)
        }
        CordsCheck {
            text: "Disable avatar animation"
            checked: !!page.draft.disableAnimations
            onToggled: page.change("disableAnimations", checked)
        }
        CordsLabel {
            text: "Notifications"
            font.pixelSize: Theme.headingSize
        }
        CordsLabel {
            text: page.desktop.state.notificationStatus || "Native presenter is starting…"
            color: Theme.secondary
            Layout.fillWidth: true
        }
        CordsCheck {
            text: "Notify for new messages"
            checked: !!page.draft.notifications
            onToggled: page.change("notifications", checked)
        }
        CordsCheck {
            text: "Show message content in notifications"
            checked: !!page.draft.notificationPreviews
            onToggled: page.change("notificationPreviews", checked)
        }
        CordsCheck {
            text: "Do Not Disturb"
            checked: !!page.draft.doNotDisturb
            onToggled: page.change("doNotDisturb", checked)
        }
        CordsLabel {
            text: "Server and channel mute controls override these settings. Alerts require this app to remain running and unlocked. Mentions-only filtering needs structured mention support."
            color: Theme.secondary
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "Lock and privacy"
            font.pixelSize: Theme.headingSize
        }
        CordsCheck {
            text: "Genericize locked account image"
            checked: !!page.draft.genericize
            onToggled: page.change("genericize", checked)
        }
        CordsCheck {
            text: "Hide nickname while locked"
            checked: !!page.draft.hideName
            onToggled: page.change("hideName", checked)
        }
        CordsCheck {
            text: "Lock on suspend"
            checked: !!page.draft.lockOnSuspend
            onToggled: page.change("lockOnSuspend", checked)
        }
        CordsCheck {
            visible: Qt.platform.os === "windows"
            text: "Lock when the Windows session locks"
            checked: !!page.draft.lockOnOsLock
            onToggled: page.change("lockOnOsLock", checked)
        }
        CordsLabel {
            text: "Inactivity lock"
        }
        RowLayout {
            CordsButton {
                text: "5 min"
                primary: page.draft.autoLockMinutes === 5
                onClicked: page.change("autoLockMinutes", 5)
            }
            CordsButton {
                text: "15 min"
                primary: page.draft.autoLockMinutes === 15
                onClicked: page.change("autoLockMinutes", 15)
            }
            CordsButton {
                text: "60 min"
                primary: page.draft.autoLockMinutes === 60
                onClicked: page.change("autoLockMinutes", 60)
            }
            CordsButton {
                text: "Never"
                primary: page.draft.autoLockMinutes === 0
                onClicked: page.change("autoLockMinutes", 0)
            }
        }
        CordsLabel {
            text: Qt.platform.os === "windows"
                ? "Windows session-lock and suspend events use your Rust account policy."
                : "Suspend-gap and inactivity locking run in Rust. OS session-lock events are still unavailable on this platform."
            color: Theme.warning
            Layout.fillWidth: true
        }
        RowLayout {
            CordsButton {
                text: "Save preferences"
                primary: true
                enabled: page.dirty && !page.desktop.busy
                onClicked: {
                    page.saving = true;
                    page.desktop.savePreferences(page.draft);
                }
            }
            CordsButton {
                text: "Discard edits"
                enabled: page.dirty
                onClicked: page.reload()
            }
        }
        CordsLabel {
            text: "Local account password"
            font.pixelSize: Theme.headingSize
            Layout.topMargin: 12
        }
        CordsField {
            id: currentPassword
            placeholderText: "Current password"
            echoMode: TextInput.Password
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
            Layout.fillWidth: true
        }
        CordsField {
            id: newPassword
            placeholderText: "New password (at least 12 characters)"
            echoMode: TextInput.Password
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
            Layout.fillWidth: true
        }
        CordsCheck {
            id: weak
            text: "Accept an identified weak-password warning"
        }
        CordsButton {
            text: "Change local password"
            enabled: !page.desktop.busy && currentPassword.text.length > 0 && newPassword.text.length >= 12
            onClicked: {
                page.desktop.changePassword(page.desktop.state.account, currentPassword.text, newPassword.text, weak.checked);
                currentPassword.clear();
                newPassword.clear();
            }
        }
        CordsLabel {
            text: "Advanced identity and session"
            font.pixelSize: Theme.headingSize
            Layout.topMargin: 12
        }
        CordsLabel {
            text: "To remove a local vault, lock the account, select it in the account picker, and choose Remove from this device. Keep a verified recovery copy first."
            Layout.fillWidth: true
            color: Theme.secondary
        }
        CordsButton {
            text: "Lock and open account picker"
            enabled: !page.desktop.busy
            onClicked: page.desktop.lockAccount(false)
        }
        CordsField {
            text: page.desktop.state.device || ""
            readOnly: true
            Layout.fillWidth: true
            Accessible.name: "Device fingerprint"
        }
        CordsButton {
            text: "Publish fresh KeyPackage"
            enabled: !page.desktop.busy && !!page.desktop.state.server
            onClicked: page.desktop.publishKeys()
        }
        CordsButton {
            text: "Sign out and lock"
            enabled: !page.desktop.busy
            onClicked: page.desktop.lockAccount(true)
        }
        CordsButton {
            text: "Revoke this device…"
            destructive: true
            enabled: !page.desktop.busy && !!page.desktop.state.server
            onClicked: {
                confirm.title = "Revoke this device";
                confirm.explanation = "Revoke this account device. This can prevent future communication and does not erase copies already delivered.";
                confirm.operation = function () {
                    page.desktop.revokeDevice();
                };
                confirm.open();
            }
        }
        CordsButton {
            text: "Burn identity…"
            destructive: true
            enabled: !page.desktop.busy && !page.desktop.state.burned
            onClicked: {
                confirm.title = "Permanently burn identity";
                confirm.explanation = "This is terminal for this identity. Signed burn notices will be delivered to joined servers; unavailable servers remain pending. This cannot be undone.";
                confirm.operation = function () {
                    page.desktop.burnIdentity();
                };
                confirm.open();
            }
        }
        CordsButton {
            text: "Retry pending burn delivery"
            visible: !!page.desktop.state.burned
            enabled: !page.desktop.busy
            onClicked: page.desktop.retryBurns()
        }
        Item {
            Layout.preferredHeight: 20
        }
    }
    ConfirmDialog {
        id: confirm
    }
}
