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
    readonly property string scope: JSON.stringify([desktop.state.account || "", desktop.state.server || ""])
    onScopeChanged: {
        ownerCode.clear();
        successorAccount.clear();
        designation.clear();
        navigationSearch.clear();
        clearChannelDraft();
    }
    onRouteChanged: clearChannelDraft()
    function clearChannelDraft() {
        targetDevice.clear();
        replacementName.clear();
        replacementPublic.checked = false;
        confirmation.close();
        confirmation.operation = null;
    }
    function scopedOperation(action) {
        const reviewedScope = scope;
        const reviewedRoute = route;
        return function () {
            if (admin.scope === reviewedScope && admin.route === reviewedRoute)
                action();
        };
    }
    signal profileRequested(string account)
    signal closeRequested
    function requestClose() {
        if (modalOpen || (desktop.busy && serverProfileForm.dirty))
            return;
        if (serverProfileForm.dirty) {
            confirmation.title = "Discard server profile edits?";
            confirmation.explanation = "Your unsaved changes will be lost.";
            confirmation.operation = admin.scopedOperation(function () {
                serverProfileForm.reset();
                admin.closeRequested();
            });
            confirmation.open();
        } else
            closeRequested();
    }
    signal claimRequested
    property int page: 0
    onVisibleChanged: if (visible && !desktop.busy && !serverProfileForm.dirty)
        selectPage(page)
    readonly property bool modalOpen: serverProfileForm.modalOpen || requestsWorkspace.modalOpen || rolesWorkspace.modalOpen || membersWorkspace.modalOpen || confirmation.visible
    function selectPage(index) {
        page = index;
        if (desktop.busy)
            return;
        if (page === 0 && desktop.state.admission === "active")
            desktop.loadServerProfile();
        else if (page === 1)
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
                        enabled: !admin.desktop.busy && !serverProfileForm.dirty
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
            visible: admin.page !== 1 && admin.page !== 2 && admin.page !== 3
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
                ServerProfileForm {
                    id: serverProfileForm
                    desktop: admin.desktop
                    visible: admin.page === 0
                    Layout.fillWidth: true
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
                        objectName: "targetDevice"
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
                                confirmation.operation = admin.scopedOperation(function () {
                                    admin.desktop.removeDevice(route, device);
                                });
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
                        objectName: "replacementName"
                        placeholderText: "Successor channel name"
                        maximumLength: 100
                        Layout.fillWidth: true
                    }
                    CordsCheck {
                        id: replacementPublic
                        objectName: "replacementPublic"
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
                            confirmation.operation = admin.scopedOperation(function () {
                                admin.desktop.replaceChannel(route, name, isPublic);
                            });
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
                            confirmation.operation = admin.scopedOperation(function () {
                                admin.desktop.retireChannel(route);
                            });
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
                        objectName: "ownerCode"
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
                                confirmation.operation = admin.scopedOperation(function () {
                                    admin.desktop.recoverOwnership(ownerCode.text);
                                    ownerCode.clear();
                                });
                                confirmation.open();
                            }
                        }
                    }
                    CordsField {
                        id: successorAccount
                        objectName: "successorAccount"
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
                            confirmation.operation = admin.scopedOperation(function () {
                                admin.desktop.designateSuccessor(account);
                            });
                            confirmation.open();
                        }
                    }
                    CordsField {
                        id: designation
                        objectName: "designation"
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
        RequestsWorkspace {
            id: requestsWorkspace
            desktop: admin.desktop
            visible: admin.page === 2
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 24
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
        objectName: "serverAdministrationConfirmation"
    }
}
