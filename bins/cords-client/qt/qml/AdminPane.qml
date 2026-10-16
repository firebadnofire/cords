pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: admin
    required property Controller desktop
    property string route: ""
    signal profileRequested(string account)
    property int page: 0
    property var editingRole: ({})
    property string mutationId: ""
    function selectPage(index) {
        page = index;
        if (desktop.busy)
            return;
        if (page === 1)
            desktop.loadDirectory("");
        else if (page === 2)
            desktop.loadRequests();
        else if (page === 3)
            desktop.loadRoles();
        else if (page === 5)
            desktop.loadAudit(0);
    }
    function roleDraft(role) {
        editingRole = Object.assign({}, role);
        mutationId = desktop.newId();
        roleName.text = role.name;
        roleColor.text = role.color;
        roleDescription.text = role.description;
        rolePermissions.text = (role.permissions || []).join("\n");
        mentionable.checked = !!role.mentionable;
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        Flow {
            Layout.fillWidth: true
            spacing: 6
            Repeater {
                model: ["Overview", "Members", "Requests", "Roles", "Channels", "Audit", "Ownership"]
                delegate: CordsButton {
                    required property string modelData
                    required property int index
                    text: modelData
                    primary: admin.page === index
                    implicitWidth: Math.max(54, contentItem.implicitWidth + 16)
                    onClicked: admin.selectPage(index)
                }
            }
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            ColumnLayout {
                width: parent.width
                spacing: 12
                ColumnLayout {
                    visible: admin.page === 0
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Server overview"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsField {
                        text: admin.desktop.state.origin || ""
                        readOnly: true
                        Layout.fillWidth: true
                        Accessible.name: "Server HTTPS origin"
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
                        text: "Authenticate / refresh admission"
                        enabled: !admin.desktop.busy && !!admin.desktop.state.server
                        onClicked: admin.desktop.authenticate()
                    }
                    CordsLabel {
                        text: "Server presentation editing is unavailable in the current Rust client API."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    visible: admin.page === 1
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Admitted accounts"
                        font.pixelSize: Theme.headingSize
                    }
                    CordsField {
                        id: memberSearch
                        placeholderText: "Find account"
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: admin.page === 1 ? admin.desktop.members : null
                        delegate: ColumnLayout {
                            required property var row
                            Layout.fillWidth: true
                            visible: !memberSearch.text || row.name.toLowerCase().includes(memberSearch.text.toLowerCase()) || row.id.includes(memberSearch.text)
                            CordsButton {
                                text: parent.row.name
                                Layout.fillWidth: true
                                onClicked: admin.profileRequested(parent.row.id)
                            }
                            CordsButton {
                                text: "Read role assignments"
                                enabled: !admin.desktop.busy
                                onClicked: admin.desktop.loadAccountRoles(parent.row.id)
                            }
                            CordsLabel {
                                text: "Assigned roles: " + (parent.row.roles || []).join(", ")
                                Layout.fillWidth: true
                                color: Theme.secondary
                            }
                        }
                    }
                    CordsButton {
                        text: "Refresh directory"
                        enabled: !admin.desktop.busy
                        onClicked: admin.desktop.loadDirectory("")
                    }
                    CordsButton {
                        text: "Load more accounts"
                        visible: !!admin.desktop.state.directoryAfter
                        enabled: !admin.desktop.busy
                        onClicked: admin.desktop.loadDirectory(admin.desktop.state.directoryAfter)
                    }
                    CordsLabel {
                        text: "Authorized device diagnostics"
                        font.pixelSize: Theme.headingSize
                        Layout.topMargin: 12
                    }
                    Repeater {
                        model: admin.page === 1 ? admin.desktop.devices : null
                        delegate: CordsLabel {
                            required property var row
                            text: row.name + "\nAccount: " + row.account + "\nDevice: " + row.id + " · generation " + row.generation
                            Layout.fillWidth: true
                            color: Theme.secondary
                        }
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
                                    enabled: !admin.desktop.busy && request.row.status === "pending"
                                    onClicked: admin.desktop.approveRequest(request.row.id)
                                }
                                CordsButton {
                                    text: "Reject…"
                                    destructive: true
                                    enabled: !admin.desktop.busy && request.row.status === "pending"
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
                    visible: admin.page === 3
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "Roles · policy revision " + (admin.desktop.state.revision || 0)
                        font.pixelSize: Theme.headingSize
                    }
                    CordsLabel {
                        text: admin.desktop.state.rolesActive ? "Server-enforced policy" : "Preparatory role catalog. Saving roles does not activate channel permission enforcement."
                        color: Theme.warning
                        Layout.fillWidth: true
                    }
                    RowLayout {
                        CordsButton {
                            text: "Refresh roles"
                            enabled: !admin.desktop.busy
                            onClicked: admin.desktop.loadRoles()
                        }
                        CordsButton {
                            text: "New role"
                            enabled: !admin.desktop.busy
                            onClicked: admin.roleDraft({
                                id: admin.desktop.newId(),
                                name: "",
                                color: "#80b5db",
                                description: "",
                                priority: 1,
                                permissions: [],
                                mentionable: false
                            })
                        }
                    }
                    Repeater {
                        model: admin.page === 3 ? admin.desktop.roles : null
                        delegate: CordsButton {
                            required property var row
                            text: "$" + row.name
                            Layout.fillWidth: true
                            onClicked: admin.roleDraft(row)
                        }
                    }
                    CordsField {
                        id: roleName
                        placeholderText: "Role name (without $)"
                        maximumLength: 100
                        Layout.fillWidth: true
                    }
                    CordsField {
                        id: roleColor
                        placeholderText: "Role color (#RRGGBB)"
                        Layout.fillWidth: true
                    }
                    CordsField {
                        id: roleDescription
                        placeholderText: "Role description"
                        Layout.fillWidth: true
                    }
                    RowLayout {
                        CordsLabel {
                            text: "Priority: " + (admin.editingRole.priority || 0)
                        }
                        CordsButton {
                            text: "Move up"
                            onClicked: {
                                let draft = Object.assign({}, admin.editingRole);
                                draft.priority = (draft.priority || 0) + 1;
                                admin.editingRole = draft;
                            }
                        }
                        CordsButton {
                            text: "Move down"
                            enabled: (admin.editingRole.priority || 0) > 0
                            onClicked: {
                                let draft = Object.assign({}, admin.editingRole);
                                draft.priority -= 1;
                                admin.editingRole = draft;
                            }
                        }
                    }
                    CordsLabel {
                        text: "Capability strings, one per line. The server validates grants, hierarchy and authority."
                        Layout.fillWidth: true
                        color: Theme.secondary
                    }
                    TextArea {
                        id: rolePermissions
                        Layout.fillWidth: true
                        Layout.preferredHeight: 120
                        textFormat: TextEdit.PlainText
                        color: Theme.text
                        selectByMouse: true
                        background: Rectangle {
                            color: Theme.rail
                            border.color: Theme.border
                        }
                        Accessible.name: "Role capability strings"
                    }
                    CordsCheck {
                        id: mentionable
                        text: "Mentionable role"
                    }
                    RowLayout {
                        CordsButton {
                            text: "Save role"
                            primary: true
                            enabled: !admin.desktop.busy && !!admin.editingRole.id && roleName.text.trim().length > 0
                            onClicked: admin.desktop.putRole({
                                id: admin.editingRole.id,
                                name: roleName.text,
                                color: roleColor.text,
                                description: roleDescription.text,
                                priority: admin.editingRole.priority || 0,
                                permissions: rolePermissions.text.split("\n").map(s => s.trim()).filter(s => s.length > 0),
                                mentionable: mentionable.checked
                            }, admin.desktop.state.revision, admin.mutationId)
                        }
                        CordsButton {
                            text: "Delete role…"
                            destructive: true
                            enabled: !admin.desktop.busy && !!admin.editingRole.id && !admin.editingRole.id.startsWith("builtin:")
                            onClicked: {
                                confirmation.title = "Delete role";
                                confirmation.explanation = "Delete $" + roleName.text + " at the reviewed policy revision.";
                                const id = admin.editingRole.id;
                                const revision = admin.desktop.state.revision;
                                const requestId = admin.desktop.newId();
                                confirmation.operation = function () {
                                    admin.desktop.deleteRole(id, revision, requestId);
                                };
                                confirmation.open();
                            }
                        }
                    }
                    CordsLabel {
                        text: "Account role assignment"
                        font.pixelSize: Theme.headingSize
                        Layout.topMargin: 14
                    }
                    CordsField {
                        id: assignedAccount
                        placeholderText: "Exact admitted account ID"
                        Layout.fillWidth: true
                    }
                    CordsField {
                        id: assignedRole
                        placeholderText: "Stable role ID"
                        Layout.fillWidth: true
                    }
                    RowLayout {
                        CordsButton {
                            text: "Assign"
                            enabled: !admin.desktop.busy && assignedAccount.text.length > 0 && assignedRole.text.length > 0
                            onClicked: admin.desktop.assignRole(assignedAccount.text, assignedRole.text, false, admin.desktop.state.revision, admin.desktop.newId())
                        }
                        CordsButton {
                            text: "Remove assignment"
                            enabled: !admin.desktop.busy && assignedAccount.text.length > 0 && assignedRole.text.length > 0
                            onClicked: admin.desktop.assignRole(assignedAccount.text, assignedRole.text, true, admin.desktop.state.revision, admin.desktop.newId())
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
    }
    ConfirmDialog {
        id: confirmation
    }
}
