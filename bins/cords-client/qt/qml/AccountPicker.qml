pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Pane {
    id: picker
    required property Controller desktop
    property string selectedAccount: ""
    property string selectedLabel: ""
    property bool creating: false
    property bool migrating: false
    property bool removalSubmitted: false
    Connections {
        target: picker.desktop
        function onStateChanged() {
            if (!picker.removalSubmitted || picker.desktop.busy)
                return;
            picker.removalSubmitted = false;
            if (!picker.desktop.state.error) {
                picker.selectedAccount = "";
                picker.selectedLabel = "";
                removal.close();
            }
        }
    }
    background: Rectangle {
        color: Theme.window
    }
    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth
        ColumnLayout {
            width: Math.min(420, picker.width - 48)
            x: (picker.width - width) / 2
            spacing: 14
            Item {
                Layout.preferredHeight: Math.max(20, picker.height * 0.10)
            }
            CordsLabel {
                text: "Cords"
                font.pixelSize: 36
                font.bold: true
            }
            CordsLabel {
                text: "Right on the wire"
                color: Theme.secondary
            }
            CordsLabel {
                text: picker.migrating ? "Migrate an existing vault" : picker.creating ? "Create a local account" : "Unlock your account"
                font.pixelSize: Theme.headingSize
                Layout.topMargin: 20
            }
            CordsLabel {
                text: "Your identity and message keys stay in your encrypted local account vault."
                Layout.fillWidth: true
                color: Theme.secondary
            }
            Repeater {
                model: !picker.creating && !picker.migrating ? picker.desktop.accounts : null
                delegate: CordsButton {
                    id: accountChoice
                    required property var row
                    text: row.name
                    implicitHeight: 56
                    contentItem: RowLayout {
                        CordsAvatar {
                            name: accountChoice.row.name
                            account: accountChoice.row.id
                            source: accountChoice.row.avatar
                            shape: accountChoice.row.shape
                            centerX: accountChoice.row.xMilli / 100000
                            centerY: accountChoice.row.yMilli / 100000
                            zoom: accountChoice.row.zoomMilli / 1000
                            playing: false
                            Layout.preferredWidth: 32
                            Layout.preferredHeight: 32
                        }
                        CordsLabel {
                            text: accountChoice.text
                            Layout.fillWidth: true
                            color: accountChoice.primary ? Theme.onAccent : Theme.text
                            elide: Text.ElideRight
                            wrapMode: Text.NoWrap
                        }
                    }
                    Layout.fillWidth: true
                    primary: picker.selectedAccount === row.id
                    onClicked: {
                        picker.selectedAccount = row.id;
                        picker.selectedLabel = row.name;
                        password.forceActiveFocus();
                    }
                }
            }
            CordsField {
                id: nickname
                visible: picker.creating || picker.migrating
                placeholderText: "Local account nickname"
                maximumLength: 100
                Layout.fillWidth: true
                objectName: "accountNickname"
            }
            CordsField {
                id: oldPassword
                visible: picker.migrating
                placeholderText: "Current legacy password (if used)"
                echoMode: TextInput.Password
                Layout.fillWidth: true
                inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
            }
            CordsField {
                id: password
                objectName: "accountPassword"
                placeholderText: picker.creating || picker.migrating ? "New password (at least 12 characters)" : "Account password"
                echoMode: TextInput.Password
                inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
                Layout.fillWidth: true
                onAccepted: submit.clicked()
            }
            CordsField {
                id: confirmation
                visible: picker.creating || picker.migrating
                placeholderText: "Confirm new password"
                echoMode: TextInput.Password
                inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
                Layout.fillWidth: true
            }
            CordsCheck {
                id: allowWeak
                visible: picker.creating || picker.migrating
                text: "I accept an identified weak-password warning"
            }
            CordsLabel {
                visible: confirmation.visible && confirmation.text.length > 0 && confirmation.text !== password.text
                text: "Passwords do not match"
                color: Theme.danger
            }
            CordsButton {
                id: submit
                objectName: "accountSubmit"
                text: picker.desktop.busy ? "Working…" : picker.migrating ? "Migrate vault" : picker.creating ? "Create account" : "Unlock"
                primary: true
                Layout.fillWidth: true
                enabled: !picker.desktop.busy && password.text.length > 0 && ((picker.creating || picker.migrating) ? nickname.text.trim().length > 0 && password.text === confirmation.text : picker.selectedAccount.length > 0)
                onClicked: {
                    if (!enabled)
                        return;
                    if (picker.migrating)
                        picker.desktop.migrateAccount(nickname.text, oldPassword.text, password.text, allowWeak.checked);
                    else if (picker.creating)
                        picker.desktop.createAccount(nickname.text, password.text, allowWeak.checked);
                    else
                        picker.desktop.unlockAccount(picker.selectedAccount, password.text);
                    password.clear();
                    confirmation.clear();
                    oldPassword.clear();
                }
            }
            CordsButton {
                text: picker.creating || picker.migrating ? "Back to accounts" : "Create another account"
                Layout.fillWidth: true
                enabled: !picker.desktop.busy
                onClicked: {
                    picker.creating = !picker.creating && !picker.migrating;
                    picker.migrating = false;
                    password.clear();
                    confirmation.clear();
                    oldPassword.clear();
                }
            }
            CordsButton {
                text: "Migrate legacy account"
                visible: !!picker.desktop.state.legacy && !picker.creating && !picker.migrating
                Layout.fillWidth: true
                enabled: !picker.desktop.busy
                onClicked: picker.migrating = true
            }
            CordsButton {
                text: "Remove selected account from this device…"
                destructive: true
                visible: !picker.creating && !picker.migrating && !!picker.selectedAccount
                enabled: !picker.desktop.busy
                Layout.fillWidth: true
                onClicked: {
                    password.clear();
                    removal.open();
                }
            }
            CordsLabel {
                text: picker.desktop.state.error || ""
                color: Theme.danger
                Layout.fillWidth: true
                objectName: "accountError"
            }
            Item {
                Layout.preferredHeight: 24
            }
        }
    }
    LocalAccountRemoval {
        id: removal
        account: picker.selectedAccount
        nickname: picker.selectedLabel
        operationBusy: picker.desktop.busy
        errorText: picker.desktop.state.error || ""
        onRemovalRequested: (account, password) => {
            picker.removalSubmitted = true;
            picker.desktop.removeAccount(account, password);
        }
    }
}
