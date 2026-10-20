pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Shapes

Pane {
    id: picker
    required property Controller desktop
    property string selectedAccount: ""
    property string selectedLabel: ""
    property bool creating: false
    property bool migrating: false
    property bool removalSubmitted: false
    padding: 0
    function resetSelection() {
        selectedAccount = "";
        selectedLabel = "";
        creating = false;
        migrating = false;
        password.clear();
        confirmation.clear();
        oldPassword.clear();
        allowWeak.checked = false;
    }
    function revealLogin() {
        if (selectedAccount || creating || migrating)
            scroll.contentItem.contentY = Math.max(0, Math.min(loginPanel.y - 20, content.height - scroll.height));
    }
    onHeightChanged: Qt.callLater(revealLogin)
    function selectIdentity(account, label) {
        resetSelection();
        selectedAccount = account;
        selectedLabel = label;
        Qt.callLater(function () {
            password.forceActiveFocus();
            picker.revealLogin();
        });
    }
    Shortcut {
        sequence: "Escape"
        enabled: !picker.desktop.busy && (!!picker.selectedAccount || picker.creating || picker.migrating)
        onActivated: {
            picker.resetSelection();
            addTile.forceActiveFocus();
        }
    }
    Connections {
        target: picker.desktop
        function onStateChanged() {
            if (!picker.removalSubmitted || picker.desktop.busy)
                return;
            picker.removalSubmitted = false;
            if (!picker.desktop.state.error) {
                picker.resetSelection();
                removal.close();
            }
        }
    }
    background: Rectangle {
        color: Theme.rail
        clip: true
        gradient: Gradient {
            GradientStop {
                position: 0
                color: Theme.light ? Theme.navigation : "#071828"
            }
            GradientStop {
                position: 1
                color: Theme.rail
            }
        }
        Repeater {
            model: 3
            delegate: Shape {
                id: ribbon
                required property int index
                anchors.fill: parent
                ShapePath {
                    strokeWidth: 30
                    strokeColor: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.06 + ribbon.index * 0.025)
                    fillColor: "transparent"
                    startX: -picker.width * 0.1
                    startY: picker.height * 0.52 + ribbon.index * 32
                    PathCubic {
                        x: picker.width * 1.1
                        y: picker.height * 1.05 + ribbon.index * 32
                        control1X: picker.width * 0.08
                        control1Y: picker.height * 1.03 + ribbon.index * 32
                        control2X: picker.width * 0.67
                        control2Y: picker.height * 0.45 + ribbon.index * 32
                    }
                }
                ShapePath {
                    strokeWidth: 28
                    strokeColor: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.05 + ribbon.index * 0.02)
                    fillColor: "transparent"
                    startX: picker.width * 0.40 + ribbon.index * 26
                    startY: -100
                    PathCubic {
                        x: picker.width * 1.2
                        y: picker.height * 0.27 + ribbon.index * 28
                        control1X: picker.width * 0.37
                        control1Y: picker.height * 0.25 + ribbon.index * 28
                        control2X: picker.width * 0.85
                        control2Y: -picker.height * 0.1
                    }
                }
            }
        }
    }
    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth
        clip: true
        ColumnLayout {
            id: content
            onHeightChanged: Qt.callLater(picker.revealLogin)
            width: scroll.availableWidth
            spacing: 20
            Item {
                Layout.preferredHeight: Math.max(28, picker.height * 0.10)
            }
            CordsLabel {
                text: "Who's playing today?"
                font.pixelSize: picker.width < 800 ? 28 : 34
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            CordsLabel {
                text: "Choose your Cords identity"
                color: Theme.secondary
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            GridLayout {
                id: tiles
                visible: !picker.creating && !picker.migrating
                columns: Math.max(1, Math.min(picker.desktop.accounts.count + 1, Math.floor((picker.width - 48) / 172)))
                columnSpacing: 16
                rowSpacing: 8
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 20
                AccountTile {
                    id: addTile
                    objectName: "addAccountTile"
                    addAccount: true
                    enabled: !picker.desktop.busy
                    onClicked: {
                        picker.resetSelection();
                        picker.creating = true;
                        Qt.callLater(function () {
                            nickname.forceActiveFocus();
                        });
                    }
                }
                Repeater {
                    model: picker.desktop.accounts
                    delegate: AccountTile {
                        required property var row
                        objectName: "identityTile"
                        account: row.id
                        label: row.name
                        avatarSource: row.avatar
                        centerX: row.xMilli / 100000
                        centerY: row.yMilli / 100000
                        zoom: row.zoomMilli / 1000
                        selected: picker.selectedAccount === row.id
                        enabled: !picker.desktop.busy
                        onClicked: picker.selectIdentity(row.id, row.name)
                    }
                }
            }
            CordsLabel {
                visible: picker.desktop.accounts.count === 0 && !picker.creating && !picker.migrating
                text: "New here? Add an identity to get started. Each identity has its own servers and encrypted local vault."
                Layout.maximumWidth: 480
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignHCenter
                horizontalAlignment: Text.AlignHCenter
                color: Theme.secondary
            }
            ColumnLayout {
                id: loginPanel
                Layout.preferredWidth: Math.min(400, picker.width - 48)
                Layout.maximumWidth: Math.min(400, picker.width - 48)
                Layout.minimumWidth: 0
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignHCenter
                spacing: 12
                CordsLabel {
                    visible: !!picker.selectedAccount || picker.creating || picker.migrating
                    text: picker.migrating ? "Migrate an existing vault" : picker.creating ? "Create a local identity" : picker.selectedLabel
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: Theme.headingSize
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
                    visible: picker.creating || picker.migrating || !!picker.selectedAccount
                    placeholderText: picker.creating || picker.migrating ? "New password" : "Account password"
                    echoMode: TextInput.Password
                    inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
                    Layout.fillWidth: true
                    onAccepted: submit.clicked()
                }
                CordsField {
                    id: confirmation
                    objectName: "accountConfirmation"
                    visible: picker.creating || picker.migrating
                    placeholderText: "Confirm new password"
                    echoMode: TextInput.Password
                    inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
                    Layout.fillWidth: true
                }
                CordsCheck {
                    id: allowWeak
                    Layout.fillWidth: true
                    visible: picker.creating || picker.migrating
                    objectName: "ignoreWeakPasswords"
                    enabled: !picker.desktop.busy
                    text: "Ignore weak passwords"
                }
                CordsLabel {
                    visible: confirmation.visible && confirmation.text.length > 0 && confirmation.text !== password.text
                    text: "Passwords do not match"
                    color: Theme.danger
                }
                CordsButton {
                    id: submit
                    objectName: "accountSubmit"
                    visible: password.visible
                    text: picker.desktop.busy ? "Working…" : picker.migrating ? "Migrate vault" : picker.creating ? "Create account" : "Log in"
                    primary: true
                    Layout.fillWidth: true
                    enabled: !picker.desktop.busy && ((picker.creating || picker.migrating) ? nickname.text.trim().length > 0 && (allowWeak.checked || Array.from(password.text).length >= 12) && password.text === confirmation.text : picker.selectedAccount.length > 0)
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
                    objectName: "deselectAccount"
                    text: "Back to identities"
                    visible: !!picker.selectedAccount || picker.creating || picker.migrating
                    Layout.fillWidth: true
                    enabled: !picker.desktop.busy
                    onClicked: {
                        picker.resetSelection();
                        addTile.forceActiveFocus();
                    }
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
            }
            Item {
                Layout.preferredHeight: 28
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
