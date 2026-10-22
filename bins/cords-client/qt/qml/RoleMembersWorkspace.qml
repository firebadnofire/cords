pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: workspace
    objectName: "roleMembersWorkspace"
    required property Controller desktop
    property string roleId: ""
    property bool saved: false
    property bool blocked: false
    property bool adding: false
    property string scope: ""
    property var selected: []
    property var selectionRevision: 0
    property string stage: ""
    property string message: ""
    property var batch: []
    property int confirmed: 0
    property bool failed: false
    property bool removing: false
    property bool pendingRead: false
    property var autoRevision: -1
    signal membershipChanged(var revision)
    readonly property string currentScope: JSON.stringify([desktop.state.account, desktop.state.server, roleId])
    readonly property var page: desktop.state.roleMembers || ({})
    readonly property bool current: saved && !!page.loaded && page.role === roleId && page.assigned === !adding && page.revision === desktop.state.revision && scope === currentScope
    readonly property bool actionable: current && !blocked && !desktop.busy && !stage && !failed
    readonly property bool modalOpen: removal.visible
    readonly property var displayedRows: {
        const people = desktop.state.people || ({});
        const term = search.text.trim().toLocaleLowerCase();
        return (current ? page.rows || [] : []).filter(row => !term || (row.account + " " + (people[row.account] || ({})).name).toLocaleLowerCase().includes(term));
    }
    spacing: 10
    function reset() {
        scope = currentScope;
        adding = false;
        selected = [];
        batch = [];
        confirmed = 0;
        failed = false;
        stage = "";
        message = "";
        autoRevision = -1;
        search.clear();
        removal.close();
        pendingRead = visible && saved;
        Qt.callLater(readPending);
    }
    function readPending() {
        if (pendingRead && visible && saved && !desktop.busy && !stage) {
            pendingRead = false;
            reload("");
        }
    }
    function reload(after) {
        if (!saved || !visible || desktop.busy || stage || scope !== currentScope) return;
        stage = "read";
        autoRevision = desktop.state.revision;
        desktop.loadRoleMembers(roleId, !adding, after);
    }
    function setAdding(value) {
        if (desktop.busy || stage || failed) return;
        adding = value;
        selected = [];
        search.clear();
        reload("");
    }
    function selectAccount(account, checked) {
        if (!actionable || !adding) return;
        const row = (page.rows || []).find(row => row.account === account);
        if (!row || !row.canAssign) return;
        let next = selected.filter(id => id !== account);
        if (checked) {
            if (next.length >= 64) { message = "Select up to 64 accounts at a time."; return; }
            next.push(account);
        }
        selected = next;
        selectionRevision = page.revision;
    }
    function start(accounts, remove) {
        if (!actionable || !accounts.length || accounts.length > 64 || (!remove && selectionRevision !== page.revision)) return;
        if (remove && !accounts.every(account => (page.rows || []).some(row => row.account === account && row.canRemove))) return;
        batch = accounts.map(account => ({account: account, requestId: desktop.newId(), revision: -1}));
        confirmed = 0;
        removing = remove;
        failed = false;
        message = "";
        writeNext();
    }
    function writeNext() {
        if (!visible || blocked || scope !== currentScope || desktop.busy || stage || confirmed >= batch.length) return;
        let next = batch.slice();
        if (next[confirmed].revision < 0) next[confirmed].revision = desktop.state.revision;
        batch = next;
        failed = false;
        stage = "write";
        const item = batch[confirmed];
        desktop.assignRole(item.account, roleId, removing, item.revision, item.requestId);
    }
    onRoleIdChanged: reset()
    onVisibleChanged: {
        if (visible) reset();
        else { pendingRead = false; removal.close(); }
    }
    Connections {
        target: workspace.desktop
        function onBusyChanged() { Qt.callLater(workspace.readPending); }
        function onStateChanged() {
            if (workspace.scope !== workspace.currentScope) { workspace.reset(); return; }
            if (!workspace.visible || workspace.desktop.busy) return;
            if (!workspace.stage) {
                if (workspace.saved && !workspace.blocked && !workspace.failed && workspace.autoRevision !== workspace.desktop.state.revision && !(workspace.batch.length && workspace.confirmed < workspace.batch.length)) {
                    workspace.pendingRead = true;
                    Qt.callLater(workspace.readPending);
                }
                return;
            }
            const stage = workspace.stage;
            workspace.stage = "";
            if (stage === "write") {
                if (workspace.desktop.state.error) {
                    workspace.failed = true;
                    workspace.message = "Confirmed " + workspace.confirmed + " of " + workspace.batch.length + ". Stopped at " + workspace.batch[workspace.confirmed].account + ": " + workspace.desktop.state.error + ". Retry keeps the same request receipt; refresh shows current assignments.";
                    Qt.callLater(function () { workspace.reload(""); });
                    return;
                }
                ++workspace.confirmed;
                workspace.membershipChanged(workspace.desktop.state.revision);
                workspace.message = "Confirmed " + workspace.confirmed + " of " + workspace.batch.length + ". Each account is saved separately.";
                // Queue the next account before another edit/navigation event
                // can interleave with a partially completed selection.
                if (workspace.confirmed < workspace.batch.length) workspace.writeNext();
                else { workspace.selected = []; Qt.callLater(function () { workspace.reload(""); }); }
            } else if (workspace.desktop.state.error) {
                workspace.message = workspace.desktop.state.error;
            } else if (workspace.current && workspace.selected.length && workspace.selectionRevision !== workspace.page.revision) {
                workspace.selected = [];
                workspace.message = "Policy changed. Review the current members before selecting again.";
            }
        }
    }
    RowLayout {
        Layout.fillWidth: true
        CordsButton { text: "Members"; selected: !workspace.adding; enabled: !workspace.desktop.busy && !workspace.stage && !workspace.failed; onClicked: workspace.setAdding(false) }
        CordsButton { objectName: "addRoleMembers"; text: "Add members"; selected: workspace.adding; visible: workspace.roleId !== "builtin:default" && !!workspace.desktop.state.canAssign; enabled: workspace.saved && !workspace.blocked && !workspace.desktop.busy && !workspace.stage && !workspace.failed; onClicked: workspace.setAdding(true) }
        Item { Layout.fillWidth: true }
        CordsButton { objectName: "refreshRoleMembers"; text: "Refresh"; enabled: workspace.saved && !workspace.desktop.busy && !workspace.stage; onClicked: workspace.reload("") }
    }
    CordsLabel {
        Layout.fillWidth: true
        text: !workspace.saved ? "Save the role before managing members." : workspace.blocked ? "Save or discard your role edits before changing membership." : workspace.current ? workspace.page.memberCount + " assigned accounts. Search covers this page; use Next page to continue." : "Load current members to review assignments."
        color: Theme.secondary
    }
    CordsField { id: search; objectName: "roleMemberSearch"; Layout.fillWidth: true; placeholderText: "Search this page by name or account ID"; Accessible.name: placeholderText; enabled: workspace.current }
    CordsLabel { Layout.fillWidth: true; text: workspace.message; visible: !!text; color: workspace.failed ? Theme.danger : Theme.secondary }
    CordsLabel { Layout.fillWidth: true; text: workspace.desktop.state.presentationError || ""; visible: !!text; color: Theme.warning }
    ListView {
        id: list
        objectName: "roleMemberList"
        Layout.fillWidth: true
        Layout.preferredHeight: 300
        clip: true
        model: workspace.displayedRows
        spacing: 6
        delegate: RowLayout {
            id: member
            required property var modelData
            readonly property var person: (workspace.desktop.state.people || ({}))[modelData.account] || ({})
            width: list.width
            height: Math.max(64, labels.implicitHeight + 12)
            CordsAvatar {
                name: member.person.name || "Member"
                account: member.modelData.account
                source: member.person.image || ""
                shape: member.person.shape || "circle"
                playing: member.visible && member.y + member.height >= list.contentY && member.y <= list.contentY + list.height
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
            }
            ColumnLayout {
                id: labels
                Layout.fillWidth: true
                CordsLabel { text: member.person.name || "Member"; Layout.fillWidth: true; maximumLineCount: 2; elide: Text.ElideRight }
                CordsLabel { text: member.modelData.account; color: Theme.secondary; font.pixelSize: Theme.captionSize; Layout.fillWidth: true; maximumLineCount: 2; elide: Text.ElideMiddle }
            }
            CordsCheck {
                objectName: "selectRoleMember"
                visible: workspace.adding
                text: "Select"
                checked: workspace.selected.includes(member.modelData.account)
                enabled: workspace.actionable && !!member.modelData.canAssign && (checked || workspace.selected.length < 64)
                Accessible.name: "Select " + (member.person.name || member.modelData.account)
                onToggled: workspace.selectAccount(member.modelData.account, checked)
            }
            CordsButton {
                objectName: "removeRoleMember"
                visible: !workspace.adding && !!member.modelData.canRemove
                text: "Remove"
                enabled: workspace.actionable
                onClicked: { removal.account = member.modelData.account; removal.open(); }
            }
        }
        CordsLabel { anchors.centerIn: parent; width: parent.width; horizontalAlignment: Text.AlignHCenter; text: workspace.stage === "read" ? "Loading members…" : workspace.current && !list.count ? "No matching accounts on this page." : "" }
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        CordsButton { objectName: "nextRoleMemberPage"; text: "Next page"; visible: workspace.current && !!workspace.page.nextAfter; enabled: !workspace.desktop.busy && !workspace.stage && !workspace.failed; onClicked: workspace.reload(workspace.page.nextAfter) }
        CordsButton { objectName: "assignSelectedRoleMembers"; text: "Add " + workspace.selected.length + " selected"; visible: workspace.adding; primary: true; enabled: workspace.actionable && workspace.selected.length > 0; onClicked: workspace.start(workspace.selected, false) }
        CordsButton { objectName: "retryRoleMemberChange"; text: "Retry stopped account"; visible: workspace.failed; enabled: !workspace.blocked && !workspace.desktop.busy && !workspace.stage; onClicked: workspace.writeNext() }
        CordsButton { text: "Finish review"; visible: workspace.failed; enabled: !workspace.desktop.busy && !workspace.stage; onClicked: { workspace.failed = false; workspace.batch = []; workspace.selected = []; workspace.reload(""); } }
    }
    ConfirmDialog {
        id: removal
        title: "Remove role from this account?"
        property string account: ""
        explanation: account + " will lose this role. The server checks current authority and policy."
        confirmText: "Remove role"
        operation: function () { workspace.start([account], true); }
    }
}
