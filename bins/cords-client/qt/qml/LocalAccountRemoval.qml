import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CordsDialog {
    id: removal
    property string account: ""
    property string nickname: ""
    property bool operationBusy: false
    property string errorText: ""
    signal removalRequested(string account, string password)
    title: "Remove local account"
    closePolicy: removal.operationBusy ? Popup.NoAutoClose : Popup.CloseOnEscape
    onClosed: removalPassword.clear()
    ColumnLayout {
        width: parent.width
        spacing: 14
        CordsLabel {
            text: "Remove “" + removal.nickname + "” from this device?"
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "This permanently deletes this device’s encrypted local vault, cached history and key material. Keep a verified recovery copy before continuing. It does not delete the identity on other devices, revoke this device or burn the account root."
            Layout.fillWidth: true
        }
        CordsField {
            id: removalPassword
            objectName: "localRemovalPassword"
            placeholderText: "Confirm with this account’s local password"
            echoMode: TextInput.Password
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
            enabled: !removal.operationBusy
            Layout.fillWidth: true
        }
        CordsLabel {
            text: removal.errorText
            visible: text.length > 0
            color: Theme.danger
            Accessible.role: Accessible.AlertMessage
            Layout.fillWidth: true
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            CordsButton {
                objectName: "cancelRemoval"
                text: "Cancel"
                enabled: !removal.operationBusy
                onClicked: removal.close()
            }
            CordsButton {
                objectName: "confirmRemoval"
                text: removal.operationBusy ? "Removing…" : "Delete local vault"
                destructive: true
                enabled: !removal.operationBusy && !!removal.account
                onClicked: {
                    removal.removalRequested(removal.account, removalPassword.text);
                    removalPassword.clear();
                }
            }
        }
    }
}
