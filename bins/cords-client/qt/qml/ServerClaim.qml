import QtQuick
import QtQuick.Layouts

CordsDialog {
    id: claim
    property string server: ""
    property string ownership: ""
    property string origin: ""
    property bool operationBusy: false
    property string errorText: ""
    property string promptedServer: ""
    signal claimRequested(string code)
    title: "Claim this server"
    function updatePrompt() {
        if (!server || ownership !== "UNCLAIMED") {
            promptedServer = "";
            close();
        } else if (!operationBusy && promptedServer !== server) {
            close();
            promptedServer = server;
            open();
        }
    }
    onServerChanged: {
        claimCode.clear();
        updatePrompt();
    }
    onOwnershipChanged: updatePrompt()
    onOperationBusyChanged: updatePrompt()
    Component.onCompleted: updatePrompt()
    onClosed: claimCode.clear()
    ColumnLayout {
        width: parent.width
        spacing: 14
        CordsLabel {
            text: claim.origin + " has no owner yet. Claim it before anyone can request admission."
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "If you operate this server, enter the one-time claim code printed in its initial startup log. Claiming makes your current Cords identity the server owner. If you are joining someone else’s server, ask its operator to claim it first."
            Layout.fillWidth: true
        }
        CordsField {
            id: claimCode
            objectName: "serverClaimCode"
            placeholderText: "One-time server claim code"
            echoMode: TextInput.Password
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
            enabled: !claim.operationBusy
            Layout.fillWidth: true
        }
        CordsLabel {
            text: claim.errorText === "CORDS_SERVER_UNCLAIMED" ? "" : claim.errorText
            visible: text.length > 0
            color: Theme.danger
            Layout.fillWidth: true
        }
        CordsButton {
            objectName: "submitServerClaim"
            text: claim.operationBusy ? "Working…" : "Claim server"
            primary: true
            enabled: !claim.operationBusy && !!claim.server && claim.ownership === "UNCLAIMED" && claimCode.text.trim().length > 0
            onClicked: {
                claim.claimRequested(claimCode.text.trim());
                claimCode.clear();
            }
        }
        CordsButton {
            objectName: "dismissServerClaim"
            text: "Close"
            onClicked: claim.close()
        }
        CordsLabel {
            text: "You can reopen this prompt with Authenticate in the server sidebar."
            color: Theme.secondary
            Layout.fillWidth: true
        }
    }
}
