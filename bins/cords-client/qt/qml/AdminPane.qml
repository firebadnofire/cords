pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: admin
    objectName: "serverSettings"
    required property Controller desktop
    property string route: ""
    signal profileRequested(string account)
    signal claimRequested
    property int page: 0
    readonly property bool modalOpen: rolesWorkspace.modalOpen || membersWorkspace.modalOpen || confirmation.visible
    function selectPage(index) {
        page = index;
        if (desktop.busy)
            return;
        if (page === 1)
            desktop.loadDirectory("");
        else if (page === 2 && desktop.state.canApprove)
            desktop.loadRequests();
        else if (page === 3)
            desktop.loadRoles();
        else if (page === 5 && desktop.state.canAudit)
            desktop.loadAudit(0);
    }
    RowLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.preferredWidth: admin.width < 800 ? 180 : 220
            Layout.fillHeight: true
            color: Theme.navigation
            ColumnLayout {
                anchors.top: parent.top
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                width: Math.min(parent.width - 32, 220)
                anchors.margins: 16
                spacing: 4
                CordsLabel {
                    text: admin.desktop.state.origin ? admin.desktop.state.origin.replace("https://", "") : "SERVER SETTINGS"
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                    font.bold: true
                    Layout.fillWidth: true
                    Layout.bottomMargin: 14
                    elide: Text.ElideRight
                    wrapMode: Text.NoWrap
                }
                CordsField {
                    id: navigationSearch
                    placeholderText: "Find a setting"
                    Accessible.name: "Search server settings"
                    Layout.fillWidth: true
                }
                Repeater {
                    model: ["Server profile", "Members", "Requests", "Roles & permissions", "Channels", "Audit", "Ownership", "Access"]
                    delegate: CordsButton {
                        required property string modelData
                        required property int index
                        text: modelData
                        quiet: true
                        selected: admin.page === index
                        enabled: !admin.desktop.busy
                        visible: !navigationSearch.text || modelData.toLowerCase().includes(navigationSearch.text.toLowerCase())
                        Layout.fillWidth: true
                        onClicked: admin.selectPage(index)
                    }
                }
                Item {
                    Layout.fillHeight: true
                }
                CordsLabel {
                    visible: admin.height >= 620
                    text: "Server authority and conversation encryption are managed separately."
                    color: Theme.muted
                    font.pixelSize: Theme.captionSize
                    Layout.fillWidth: true
                }
            }
        }
        ScrollView {
            id: adminScroll
            visible: admin.page !== 1 && admin.page !== 3
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            topPadding: 28
            bottomPadding: 24
            ColumnLayout {
                width: Math.min(740, adminScroll.availableWidth - 48)
                x: 24
                spacing: 20
                ColumnLayout {
                    visible: admin.page === 0
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Server profile"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsLabel {
                        text: "Manage this community’s members, roles, channels and ownership."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                    CordsLabel {
                        text: "SERVER ADDRESS"
                        font.pixelSize: Theme.captionSize
                        font.bold: true
                    }
                    CordsField {
                        text: admin.desktop.state.origin || ""
                        readOnly: true
                        Layout.fillWidth: true
                        Accessible.name: "Server HTTPS origin"
                    }
                    CordsLabel {
                        text: "PINNED SIGNING IDENTITY"
                        font.pixelSize: Theme.captionSize
                        font.bold: true
                    }
                    CordsField {
                        text: admin.desktop.state.server || ""
                        readOnly: true
                        Layout.fillWidth: true
                        Accessible.name: "Pinned server signing identity"
                    }
                    CordsButton {
                        text: "Copy signing identity"
                        onClicked: admin.desktop.copy(admin.desktop.state.server)
                    }
                    CordsLabel {
                        text: "Ownership: " + (admin.desktop.state.ownership || "No server") + "\nAdmission: " + (admin.desktop.state.admission || "Not authenticated")
                        Layout.fillWidth: true
                    }
                    CordsButton {
                        text: admin.desktop.state.ownership === "UNCLAIMED" ? "Claim this server" : "Authenticate / refresh admission"
                        enabled: !admin.desktop.busy && !!admin.desktop.state.server
                        onClicked: {
                            if (admin.desktop.state.ownership === "UNCLAIMED")
                                admin.claimRequested();
                            else
                                admin.desktop.authenticate();
                        }
                    }
                    CordsLabel {
                        text: "Server name, icon and banner editing are not available yet."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    visible: admin.page === 2
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Membership requests"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsButton {
                        text: "Refresh requests"
                        enabled: !admin.desktop.busy
                        onClicked: admin.desktop.loadRequests()
                        visible: !!admin.desktop.state.canApprove
                    }
                    Repeater {
                        model: admin.page === 2 ? admin.desktop.requests : null
                        delegate: ColumnLayout {
                            id: request
                            required property var row
                            Layout.fillWidth: true
                            CordsLabel {
                                text: request.row.name + " · " + request.row.status + "\nAccount: " + request.row.account + "\nDevice: " + request.row.id
                                Layout.fillWidth: true
                            }
                            RowLayout {
                                CordsButton {
                                    text: "Approve device"
                                    enabled: !admin.desktop.busy && !!admin.desktop.state.canApprove && request.row.status === "pending"
                                    onClicked: admin.desktop.approveRequest(request.row.id)
                                }
                                CordsButton {
                                    text: "Reject…"
                                    destructive: true
                                    enabled: !admin.desktop.busy && !!admin.desktop.state.canApprove && request.row.status === "pending"
                                    onClicked: {
                                        confirmation.title = "Reject membership request";
                                        confirmation.explanation = "Reject the verified request for device " + request.row.id + ". The device may submit a fresh request.";
                                        confirmation.operation = function () {
                                            admin.desktop.rejectRequest(request.row.id);
                                        };
                                        confirmation.open();
                                    }
                                }
                            }
                        }
                    }
                }
                ColumnLayout {
                    visible: admin.page === 4
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Channel lifecycle and encrypted roster"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsField {
                        text: admin.route
                        readOnly: true
                        Layout.fillWidth: true
                        Accessible.name: "Selected channel ID"
                    }
                    CordsField {
                        id: targetDevice
                        placeholderText: "Exact authorized device ID"
                        Layout.fillWidth: true
                    }
                    RowLayout {
                        CordsButton {
                            text: "Add device"
                            enabled: !admin.desktop.busy && !!admin.route && !!targetDevice.text
                            onClicked: admin.desktop.addDevice(admin.route, targetDevice.text)
                        }
                        CordsButton {
                            text: "Remove device…"
                            destructive: true
                            enabled: !admin.desktop.busy && !!admin.route && !!targetDevice.text
                            onClicked: {
                                confirmation.title = "Remove channel device";
                                confirmation.explanation = "Remove " + targetDevice.text + " from channel " + admin.route + ". New keys require a successful MLS commit.";
                                const route = admin.route;
                                const device = targetDevice.text;
                                confirmation.operation = function () {
                                    admin.desktop.removeDevice(route, device);
                                };
                                confirmation.open();
                            }
                        }
                    }
                    CordsLabel {
                        text: "Replacement creates a new channel identity and new cryptographic group. It does not transfer old keys or history."
                        color: Theme.warning
                        Layout.fillWidth: true
                    }
                    CordsField {
                        id: replacementName
                        placeholderText: "Successor channel name"
                        maximumLength: 100
                        Layout.fillWidth: true
                    }
                    CordsCheck {
                        id: replacementPublic
                        text: "Successor is public (server can read messages)"
                    }
                    CordsButton {
                        text: "Create authenticated successor…"
                        enabled: !admin.desktop.busy && !!admin.route && !!replacementName.text
                        onClicked: {
                            confirmation.title = "Replace channel identity";
                            confirmation.explanation = "Retire " + admin.route + " and create " + replacementName.text + " as a distinct " + (replacementPublic.checked ? "public" : "encrypted") + " channel. Old local history stays separate.";
                            const route = admin.route;
                            const name = replacementName.text;
                            const isPublic = replacementPublic.checked;
                            confirmation.operation = function () {
                                admin.desktop.replaceChannel(route, name, isPublic);
                            };
                            confirmation.open();
                        }
                    }
                    CordsButton {
                        text: "Retire selected channel…"
                        destructive: true
                        enabled: !admin.desktop.busy && !!admin.route
                        onClicked: {
                            confirmation.title = "Retire channel";
                            confirmation.explanation = "Retire channel " + admin.route + ". Its identity and local archive remain distinct from future channels.";
                            const route = admin.route;
                            confirmation.operation = function () {
                                admin.desktop.retireChannel(route);
                            };
                            confirmation.open();
                        }
                    }
                    CordsLabel {
                        text: "Category editing, resource permission overrides and moderation are unavailable through the current client API."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    visible: admin.page === 5
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Role policy audit"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsLabel {
                        text: "Bounded signed server administration metadata. This is not proof of tamper-proof global history."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: admin.page === 5 ? admin.desktop.audit : null
                        delegate: CordsLabel {
                            required property var row
                            text: "Revision " + row.id + " · " + row.operation + "\nTarget: " + row.target + "\nActor device: " + row.actor
                            Layout.fillWidth: true
                        }
                    }
                    CordsButton {
                        text: "Load next audit page"
                        enabled: !admin.desktop.busy
                        onClicked: admin.desktop.loadAudit(admin.desktop.state.auditAfter || 0)
                    }
                }
                ColumnLayout {
                    visible: admin.page === 7
                    Layout.fillWidth: true
                    spacing: 16
                    CordsLabel {
                        text: "Server access"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsLabel {
                        text: "Joining the server"
                        font.bold: true
                    }
                    CordsLabel {
                        text: (admin.desktop.state.joinPolicy || []).length ? "Advertised policy: " + admin.desktop.state.joinPolicy.join(", ") : "No verified admission policy available."
                        Layout.fillWidth: true
                    }
                    CordsLabel {
                        text: "Public admission permits joining by a known server address; it does not imply a public discovery directory. Moderator approval requires staff review before membership is issued."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                    CordsButton {
                        text: "Review membership requests"
                        visible: !!admin.desktop.state.canApprove
                        enabled: !admin.desktop.busy
                        onClicked: admin.selectPage(2)
                    }
                    CordsLabel {
                        text: "Participation after joining"
                        font.bold: true
                        Layout.topMargin: 16
                    }
                    CordsLabel {
                        text: "Server membership, channel permission and encrypted conversation membership are separate checks. Joining never supplies old message keys."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                    CordsLabel {
                        text: "Rules and invitations"
                        font.bold: true
                        Layout.topMargin: 16
                    }
                    CordsLabel {
                        text: "Policy editing, invite management and enforced rule acknowledgement are not implemented by the current client/server API. Admission policy is configured by the server operator. No age-verification control is available."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    visible: admin.page === 6
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Ownership and security"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsLabel {
                        text: "Owner authority is independent of role labels. Bootstrap and recovery codes are sensitive and remain transient."
                        Layout.fillWidth: true
                        color: Theme.secondary
                    }
                    CordsField {
                        id: ownerCode
                        placeholderText: "One-time claim or operator recovery code"
                        echoMode: TextInput.Password
                        inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
                        Layout.fillWidth: true
                    }
                    RowLayout {
                        CordsButton {
                            text: "Claim unowned server"
                            enabled: !admin.desktop.busy && !!ownerCode.text
                            onClicked: {
                                admin.desktop.claimOwnership(ownerCode.text);
                                ownerCode.clear();
                            }
                        }
                        CordsButton {
                            text: "Recover owner…"
                            enabled: !admin.desktop.busy && !!ownerCode.text
                            onClicked: {
                                confirmation.title = "Recover ownership";
                                confirmation.explanation = "Use the operator recovery code to recover ownership for this authenticated identity.";
                                confirmation.operation = function () {
                                    admin.desktop.recoverOwnership(ownerCode.text);
                                    ownerCode.clear();
                                };
                                confirmation.open();
                            }
                        }
                    }
                    CordsField {
                        id: successorAccount
                        placeholderText: "Successor account ID"
                        Layout.fillWidth: true
                    }
                    CordsButton {
                        text: "Designate successor…"
                        enabled: !admin.desktop.busy && !!successorAccount.text
                        onClicked: {
                            confirmation.title = "Designate owner successor";
                            confirmation.explanation = "Designate " + successorAccount.text + " as successor for this server.";
                            const account = successorAccount.text;
                            confirmation.operation = function () {
                                admin.desktop.designateSuccessor(account);
                            };
                            confirmation.open();
                        }
                    }
                    CordsField {
                        id: designation
                        placeholderText: "Verified designation hash"
                        Layout.fillWidth: true
                    }
                    CordsButton {
                        text: "Accept successor designation"
                        enabled: !admin.desktop.busy && !!designation.text
                        onClicked: admin.desktop.acceptSuccessor(designation.text)
                    }
                }
            }
        }
        MemberWorkspace {
            id: membersWorkspace
            desktop: admin.desktop
            visible: admin.page === 1
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 24
            onProfileRequested: account => admin.profileRequested(account)
        }
        RolesWorkspace {
            id: rolesWorkspace
            desktop: admin.desktop
            visible: admin.page === 3
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 24
        }
    }
    ConfirmDialog {
        id: confirmation
    }
}
