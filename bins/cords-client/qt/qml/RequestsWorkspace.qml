pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: workspace
    required property Controller desktop
    readonly property bool modalOpen: rejection.visible
    readonly property string scope: JSON.stringify([desktop.state.account || "", desktop.state.server || ""])
    onScopeChanged: {
        search.text = "";
        rejection.close();
        rejection.operation = function () {};
    }
    spacing: 12
    RowFilter {
        id: filtered
        sourceModel: workspace.desktop.requests
        query: search.text
    }
    CordsLabel {
        text: "Membership requests"
        font.pixelSize: Theme.headingSize
    }
    CordsLabel {
        text: workspace.desktop.state.origin || "Choose a server"
        Layout.fillWidth: true
        color: Theme.secondary
    }
    RowLayout {
        Layout.fillWidth: true
        CordsField {
            id: search
            placeholderText: "Find a name or device identifier"
            Accessible.name: "Search membership requests"
            Layout.fillWidth: true
        }
        CordsButton {
            text: workspace.desktop.busy ? "Working…" : "Refresh"
            enabled: !workspace.desktop.busy && !!workspace.desktop.state.canApprove
            onClicked: workspace.desktop.loadRequests()
        }
    }
    CordsLabel {
        Layout.fillWidth: true
        visible: !workspace.desktop.state.canApprove || !workspace.desktop.state.requestsLoaded || filtered.count === 0
        text: !workspace.desktop.state.canApprove ? "Only authorized server staff can review membership requests." : workspace.desktop.busy ? "Loading membership requests…" : !workspace.desktop.state.requestsLoaded ? (workspace.desktop.state.error ? "Unable to load requests. " + workspace.desktop.state.error + " Use Refresh to retry." : "Refresh to load membership requests.") : search.text ? "No requests match your search." : "No membership requests are awaiting review."
        color: Theme.secondary
    }
    CordsLabel {
        text: workspace.desktop.state.error || ""
        visible: !!text && !!workspace.desktop.state.requestsLoaded
        color: Theme.danger
        Layout.fillWidth: true
    }
    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        reuseItems: true
        model: workspace.desktop.state.canApprove ? filtered : null
        spacing: 12
        delegate: Rectangle {
            id: request
            required property var row
            width: list.width
            height: content.implicitHeight + 24
            color: Theme.navigation
            radius: 8
            ColumnLayout {
                id: content
                x: 12
                y: 12
                width: parent.width - 24
                RowLayout {
                    CordsAvatar {
                        name: request.row.name || "Applicant"
                        account: request.row.account
                    }
                    CordsLabel {
                        text: request.row.name || "Unnamed applicant"
                        font.bold: true
                        Layout.fillWidth: true
                    }
                }
                CordsLabel {
                    text: "Account: " + request.row.account + "\nDevice: " + request.row.id
                    Layout.fillWidth: true
                    color: Theme.secondary
                    wrapMode: Text.WrapAnywhere
                }
                CordsLabel {
                    text: request.row.status + " · " + new Date(Number(request.row.requestedAt) * 1000).toLocaleString()
                    Layout.fillWidth: true
                }
                RowLayout {
                    CordsButton {
                        text: "Approve"
                        primary: true
                        enabled: !workspace.desktop.busy && request.row.status === "pending"
                        onClicked: workspace.desktop.approveRequest(request.row.id)
                    }
                    CordsButton {
                        text: "Reject"
                        destructive: true
                        enabled: !workspace.desktop.busy && request.row.status === "pending"
                        onClicked: {
                            const device = request.row.id;
                            const scope = workspace.scope;
                            rejection.title = "Reject membership request";
                            rejection.explanation = "Reject this device's request to join? The requester will see a denial and may submit a new request.";
                            rejection.operation = function () {
                                if (workspace.scope === scope)
                                    workspace.desktop.rejectRequest(device);
                            };
                            rejection.open();
                        }
                    }
                    CordsButton {
                        text: "Copy account ID"
                        onClicked: workspace.desktop.copy(request.row.account)
                    }
                }
            }
        }
    }
    CordsLabel {
        text: "Admission requests include a verified device identity and optional name. Profile pictures and application messages are not supplied by this server API."
        Layout.fillWidth: true
        color: Theme.muted
        font.pixelSize: Theme.captionSize
    }
    ConfirmDialog {
        id: rejection
        objectName: "rejectMembershipRequest"
    }
}
