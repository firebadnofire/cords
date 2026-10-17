pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page
    objectName: "accountSettings"
    required property Controller desktop
    signal closeRequested
    readonly property bool modalOpen: confirm.visible || discardPreferences.visible
    function requestClose() {
        if (saving)
            return;
        if (dirty)
            discardPreferences.open();
        else
            closeRequested();
    }
    signal profileRequested
    property string section: "Account"
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
    onVisibleChanged: {
        if (visible && !dirty)
            reload();
        if (!visible) {
            currentPassword.clear();
            newPassword.clear();
            confirmPassword.clear();
        }
    }
    onSectionChanged: {
        currentPassword.clear();
        newPassword.clear();
        confirmPassword.clear();
    }
    Component.onCompleted: reload()
    Rectangle {
        width: page.width < 800 ? 180 : Math.max(220, (page.width - 1040) / 2 + 220)
        height: parent.height
        color: Theme.navigation
    }
    ColumnLayout {
        anchors.fill: parent
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                Layout.preferredWidth: page.width < 800 ? 148 : Math.max(188, (page.width - 1040) / 2 + 188)
                Layout.fillHeight: true
                Layout.margins: 16
                spacing: 4
                CordsLabel {
                    text: "YOUR ACCOUNT"
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                    font.bold: true
                    Layout.bottomMargin: 14
                    Layout.alignment: Qt.AlignRight
                    Layout.preferredWidth: Math.min(188, parent.width)
                }
                CordsField {
                    id: settingsSearch
                    placeholderText: "Find a setting"
                    Accessible.name: "Search account settings"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 188
                    Layout.alignment: Qt.AlignRight
                }
                Repeater {
                    model: ["Account", "Appearance", "Notifications", "Privacy", "Password", "Identity"]
                    delegate: CordsButton {
                        required property string modelData
                        text: modelData
                        quiet: true
                        selected: page.section === modelData
                        visible: !settingsSearch.text || modelData.toLowerCase().includes(settingsSearch.text.toLowerCase())
                        Layout.fillWidth: true
                        Layout.maximumWidth: 188
                        Layout.alignment: Qt.AlignRight
                        onClicked: page.section = modelData
                    }
                }
                Item {
                    Layout.fillHeight: true
                }
            }
            ScrollView {
                id: settingsScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: Math.min(740, settingsScroll.availableWidth - 48)
                    x: 24
                    y: 12
                    spacing: 20
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 14
                        visible: page.section === "Account"
                        CordsLabel {
                            text: "Your account"
                            font.pixelSize: Theme.headingSize
                            Layout.topMargin: 16
                        }
                        CordsLabel {
                            text: page.desktop.state.burned ? "Identity burned" : page.desktop.state.revoked ? "Device revoked" : "Local vault unlocked"
                            color: page.desktop.state.burned || page.desktop.state.revoked ? Theme.warning : Theme.secondary
                            Layout.fillWidth: true
                        }
                        CordsLabel {
                            text: "Your fingerprint identifies your account. A profile name or local nickname does not change it."
                            color: Theme.secondary
                            Layout.fillWidth: true
                        }
                        CordsField {
                            text: page.desktop.state.account || ""
                            readOnly: true
                            Layout.fillWidth: true
                            Accessible.name: "Account fingerprint"
                        }
                        CordsButton {
                            text: "Edit public profile"
                            enabled: !page.desktop.busy
                            onClicked: page.profileRequested()
                        }
                        CordsButton {
                            text: "Copy account fingerprint"
                            onClicked: page.desktop.copy(page.desktop.state.account)
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
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 14
                        visible: page.section === "Appearance"
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
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Compact spacing"
                            checked: !!page.draft.compact
                            onToggled: page.change("compact", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Show public member panel"
                            checked: !!page.draft.showMembers
                            onToggled: page.change("showMembers", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Reduced motion"
                            checked: !!page.draft.reducedMotion
                            onToggled: page.change("reducedMotion", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Disable avatar animation"
                            checked: !!page.draft.disableAnimations
                            onToggled: page.change("disableAnimations", checked)
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 14
                        visible: page.section === "Notifications"
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
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Notify for new messages"
                            checked: !!page.draft.notifications
                            onToggled: page.change("notifications", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Show message content in notifications"
                            checked: !!page.draft.notificationPreviews
                            onToggled: page.change("notificationPreviews", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Do Not Disturb"
                            checked: !!page.draft.doNotDisturb
                            onToggled: page.change("doNotDisturb", checked)
                        }
                        CordsLabel {
                            text: "Server and channel mute controls override these settings. Alerts require this app to remain running and unlocked. Mentions-only filtering needs structured mention support."
                            color: Theme.secondary
                            Layout.fillWidth: true
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 14
                        visible: page.section === "Privacy"
                        CordsLabel {
                            text: "Lock and privacy"
                            font.pixelSize: Theme.headingSize
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Genericize locked account image"
                            checked: !!page.draft.genericize
                            onToggled: page.change("genericize", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Hide nickname while locked"
                            checked: !!page.draft.hideName
                            onToggled: page.change("hideName", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: "Lock on suspend"
                            checked: !!page.draft.lockOnSuspend
                            onToggled: page.change("lockOnSuspend", checked)
                        }
                        CordsCheck {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            visible: Qt.platform.os === "windows"
                            text: "Lock when the Windows session locks"
                            checked: !!page.draft.lockOnOsLock
                            onToggled: page.change("lockOnOsLock", checked)
                        }
                        CordsLabel {
                            text: "Inactivity lock"
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 6
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
                            text: Qt.platform.os === "windows" ? "Windows session-lock and suspend events use your Rust account policy." : "Suspend-gap and inactivity locking run in Rust. OS session-lock events are still unavailable on this platform."
                            color: Theme.warning
                            Layout.fillWidth: true
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 14
                        visible: page.section === "Password"
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
                            onTextChanged: weak.checked = false
                            placeholderText: "New password (at least 12 characters)"
                            echoMode: TextInput.Password
                            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
                            Layout.fillWidth: true
                        }
                        CordsField {
                            id: confirmPassword
                            placeholderText: "Confirm new password"
                            echoMode: TextInput.Password
                            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
                            Layout.fillWidth: true
                            Accessible.name: "Confirm new password"
                        }
                        CordsLabel {
                            visible: !!confirmPassword.text && confirmPassword.text !== newPassword.text
                            text: "The new passwords do not match."
                            color: Theme.warning
                            Layout.fillWidth: true
                        }
                        CordsCheck {
                            id: weak
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            enabled: !page.desktop.busy && Array.from(newPassword.text).length >= 12
                            text: "Allow a predictable password (12+ characters)"
                        }
                        CordsLabel {
                            text: "At least 12 characters are required. This checkbox only accepts warnings about common passwords, repeated characters, or your nickname."
                            color: Theme.secondary
                            Layout.fillWidth: true
                        }
                        CordsButton {
                            text: "Change local password"
                            enabled: !page.desktop.busy && currentPassword.text.length > 0 && Array.from(newPassword.text).length >= 12 && newPassword.text === confirmPassword.text
                            onClicked: {
                                page.desktop.changePassword(page.desktop.state.account, currentPassword.text, newPassword.text, weak.checked);
                                currentPassword.clear();
                                newPassword.clear();
                                confirmPassword.clear();
                            }
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 14
                        visible: page.section === "Identity"
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
                    }
                }
            }
        }
        CordsLabel {
            visible: page.dirty
            text: "You have unsaved preferences."
            color: Theme.warning
            Layout.leftMargin: 16
        }
        RowLayout {
            Layout.leftMargin: page.width < 800 ? 204 : Math.max(244, (page.width - 1040) / 2 + 244)
            Layout.bottomMargin: 16
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
                enabled: page.dirty && !page.desktop.busy
                onClicked: page.reload()
            }
        }
    }
    ConfirmDialog {
        id: discardPreferences
        objectName: "discardPreferences"
        title: "Discard unsaved preferences?"
        explanation: "Your saved preferences will remain unchanged."
        confirmText: "Discard and close"
        operation: function () {
            page.reload();
            page.closeRequested();
        }
    }
    ConfirmDialog {
        id: confirm
    }
}
