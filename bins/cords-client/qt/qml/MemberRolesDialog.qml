pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CordsDialog {
    id: dialog
    required property Controller desktop
    property string account: ""
    property string serverId: ""
    property string localAccount: ""
    property var assigned: []
    property var catalog: []
    property var reviewedRevision: 0
    property string stage: ""
    property string lastPayload: ""
    property string requestId: ""
    property bool loaded: false
    title: "Member roles"
    height: Math.min(580, parent ? parent.height - 32 : 580)
    closePolicy: desktop.busy ? Popup.NoAutoClose : Popup.CloseOnEscape
    onClosed: {
        confirm.close();
        confirm.operation = function () {};
        stage = "";
    }
    function inspect(member) {
        serverId = desktop.state.server || "";
        localAccount = desktop.state.account || "";
        account = member.id;
        assigned = [];
        catalog = [];
        loaded = false;
        lastPayload = "";
        open();
        reload();
    }
    function reload() {
        stage = "read";
        loaded = false;
        desktop.loadAccountRoles(account);
    }
    function changeRole(role, remove) {
        const payload = JSON.stringify([account, role, remove, reviewedRevision]);
        if (payload !== lastPayload) {
            lastPayload = payload;
            requestId = desktop.newId();
        }
        stage = "write";
        desktop.assignRole(account, role, remove, reviewedRevision, requestId);
    }
    Connections {
        target: dialog.desktop
        function onStateChanged() {
            if (dialog.visible && (dialog.serverId !== (dialog.desktop.state.server || "") || dialog.localAccount !== (dialog.desktop.state.account || ""))) {
                dialog.close();
                return;
            }
            if (!dialog.visible || dialog.desktop.busy || !dialog.stage)
                return;
            const stage = dialog.stage;
            dialog.stage = "";
            if (dialog.desktop.state.error)
                return;
            if (stage === "write") {
                Qt.callLater(dialog.reload);
                return;
            }
            const index = dialog.desktop.members.indexOf(dialog.account);
            if (index < 0)
                return;
            dialog.assigned = dialog.desktop.members.get(index).roles || [];
            let roles = [];
            for (let i = 0; i < dialog.desktop.roles.count; ++i)
                roles.push(dialog.desktop.roles.get(i));
            dialog.catalog = roles;
            dialog.reviewedRevision = dialog.desktop.state.revision;
            dialog.loaded = true;
        }
    }
    contentItem: ColumnLayout {
        spacing: 12
        CordsLabel {
            text: dialog.account
            Layout.fillWidth: true
            color: Theme.secondary
        }
        CordsLabel {
            text: dialog.desktop.state.rolesActive ? "Default permissions apply to every admitted account. The server checks your current authority and role hierarchy." : "Preparatory role assignments only. These do not activate channel permission enforcement."
            Layout.fillWidth: true
            color: Theme.warning
        }
        CordsLabel {
            text: dialog.desktop.state.error || ""
            visible: !!text
            color: Theme.danger
            Layout.fillWidth: true
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: dialog.catalog
            spacing: 8
            delegate: RowLayout {
                id: role
                required property var modelData
                width: ListView.view.width
                CordsLabel {
                    text: "$" + role.modelData.name
                    Layout.fillWidth: true
                }
                CordsButton {
                    text: role.modelData.id === "builtin:default" ? "Automatic" : dialog.assigned.includes(role.modelData.id) ? "Remove…" : "Assign"
                    enabled: dialog.loaded && !dialog.desktop.busy && !!dialog.desktop.state.canAssign && role.modelData.id !== "builtin:default"
                    onClicked: {
                        if (dialog.assigned.includes(role.modelData.id)) {
                            confirm.explanation = "Remove $" + role.modelData.name + " from account " + dialog.account + "? This can remove access when enforced policy is active.";
                            const id = role.modelData.id;
                            confirm.operation = function () {
                                dialog.changeRole(id, true);
                            };
                            confirm.open();
                        } else
                            dialog.changeRole(role.modelData.id, false);
                    }
                }
            }
            ScrollBar.vertical: ScrollBar {}
        }
        CordsLabel {
            text: dialog.desktop.busy ? "Waiting for server…" : !dialog.loaded ? "Assignments have not loaded. Refresh to retry." : "Reviewed policy revision " + dialog.reviewedRevision
            color: Theme.secondary
            Layout.fillWidth: true
        }
        RowLayout {
            CordsButton {
                text: "Refresh"
                enabled: !dialog.desktop.busy
                onClicked: dialog.reload()
            }
            CordsButton {
                text: "Close"
                enabled: !dialog.desktop.busy
                onClicked: dialog.close()
            }
        }
    }
    ConfirmDialog {
        id: confirm
        title: "Remove role assignment"
    }
}
