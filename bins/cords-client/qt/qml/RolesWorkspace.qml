pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

RowLayout {
    id: workspace
    objectName: "rolesWorkspace"
    required property Controller desktop
    readonly property bool confirmationOpen: editor.modalOpen || discard.visible || reorder.visible
    readonly property bool editorOpen: editor.visible
    readonly property bool modalOpen: editor.visible || editor.modalOpen || discard.visible || reorder.visible
    readonly property bool dirty: editor.visible && editor.dirty
    property string reorderError: ""
    property string reorderPayload: ""
    property string reorderRequestId: ""
    function reorderProposal(source, target, before) {
        const rows = [];
        for (let i = 0; i < desktop.roles.count; ++i) rows.push(desktop.roles.get(i));
        rows.sort((a, b) => b.priority - a.priority || a.id.localeCompare(b.id));
        const from = rows.findIndex(row => row.id === source);
        const to = rows.findIndex(row => row.id === target);
        const movable = row => row && row.editable && row.id !== "builtin:default";
        if (from < 0 || to < 0 || from === to || !movable(rows[from]) || !movable(rows[to]))
            throw new Error("Choose two roles you may reorder.");
        let start = from, end = from;
        while (start > 0 && movable(rows[start - 1])) --start;
        while (end + 1 < rows.length && movable(rows[end + 1])) ++end;
        if (to < start || to > end)
            throw new Error("A protected role separates these positions.");
        const segment = rows.slice(start, end + 1);
        const moving = segment.splice(from - start, 1)[0];
        segment.splice(segment.findIndex(row => row.id === target) + (before ? 0 : 1), 0, moving);
        const lower = end + 1 < rows.length ? rows[end + 1].priority : 0;
        const upper = start > 0 ? rows[start - 1].priority : Math.min(4294967296, Math.max(rows[start].priority, lower + segment.length) + segment.length + 1);
        const step = Math.floor((upper - lower) / (segment.length + 1));
        if (step < 1)
            throw new Error("No integer priorities fit between protected roles. Edit priorities before reordering.");
        const proposal = rows.map(row => ({id: row.id, priority: row.priority}));
        segment.forEach((row, index) => { proposal[start + index] = {id: row.id, priority: lower + step * (segment.length - index)}; });
        const admin = proposal.find(row => row.id === "builtin:admin");
        const moderator = proposal.find(row => row.id === "builtin:moderator");
        if (admin && moderator && admin.priority <= moderator.priority)
            throw new Error("Admin must remain above moderator.");
        return proposal;
    }
    function moveRole(source, target, before) {
        if (desktop.busy || dirty || search.text.trim()) return;
        reorderError = "";
        try {
            const positions = reorderProposal(source, target, before);
            const revision = desktop.state.revision;
            const reviewedScope = scope;
            const payload = JSON.stringify([positions, revision]);
            if (payload !== reorderPayload) {
                reorderPayload = payload;
                reorderRequestId = desktop.newId();
            }
            const requestId = reorderRequestId;
            reorder.operation = function () {
                if (workspace.scope !== reviewedScope || workspace.desktop.busy || workspace.dirty) return;
                if (workspace.desktop.state.revision !== revision) {
                    workspace.reorderError = "Policy changed. Refresh and review the order again.";
                    return;
                }
                workspace.desktop.reorderRoles(positions, revision, requestId);
            };
            reorder.open();
        } catch (error) { reorderError = error.message; }
    }
    function moveAdjacent(source, direction) {
        const rows = [];
        for (let i = 0; i < desktop.roles.count; ++i) rows.push(desktop.roles.get(i));
        rows.sort((a, b) => b.priority - a.priority || a.id.localeCompare(b.id));
        const index = rows.findIndex(row => row.id === source);
        const target = index + direction;
        if (index >= 0 && target >= 0 && target < rows.length)
            moveRole(source, rows[target].id, direction < 0);
    }
    function leave(operation) {
        if (dirty) {
            discard.operation = function () { editor.close(); operation(); };
            discard.open();
        } else {
            editor.close();
            operation();
        }
    }
    readonly property string scope: JSON.stringify([desktop.state.account || "", desktop.state.server || ""])
    onScopeChanged: {
        reorder.close();
        reorder.operation = null;
        reorderPayload = "";
        reorderRequestId = "";
        discard.close();
        discard.operation = null;
        editor.close();
    }
    spacing: 12
    RowFilter {
        id: filtered
        sourceModel: workspace.desktop.roles
        query: search.text
        sortField: "priority"
        descending: true
    }
    ColumnLayout {
        Layout.fillHeight: true
        Layout.fillWidth: !editor.visible
        Layout.preferredWidth: editor.visible ? Math.min(220, workspace.width * 0.32) : -1
        Layout.minimumWidth: 120
        CordsLabel {
            text: "Roles & permissions"
            font.pixelSize: editor.visible ? Theme.bodySize : Theme.headingSize
        }
        CordsLabel {
            visible: !editor.visible
            text: workspace.desktop.state.rolesActive ? "Server-enforced policy · revision " + workspace.desktop.state.revision : "Preparatory role catalog · saving roles does not activate channel permission enforcement."
            color: Theme.warning
            Layout.fillWidth: true
        }
        CordsButton {
            text: "Default permissions  ›"
            Layout.fillWidth: true
            visible: !editor.visible
            help: "The default role applies to every admitted account."
            enabled: !workspace.desktop.busy && workspace.desktop.roles.indexOf("builtin:default") >= 0
            onClicked: editor.edit(workspace.desktop.roles.get(workspace.desktop.roles.indexOf("builtin:default")), true)
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            CordsField {
                id: search
                objectName: "roleSearch"
                width: Math.min(300, parent.width)
                placeholderText: "Search roles"
                Accessible.name: placeholderText
            }
            CordsButton {
                text: "Refresh"
                enabled: !workspace.desktop.busy
                onClicked: workspace.desktop.loadRoles()
            }
            CordsButton {
                objectName: "createRole"
                text: "Create role"
                primary: true
                enabled: !workspace.desktop.busy && !!workspace.desktop.state.canCreateRoles
                onClicked: editor.edit({
                    id: workspace.desktop.newId(),
                    name: "",
                    color: "#8dc4e7",
                    description: "",
                    priority: 1,
                    permissions: [],
                    mentionable: false
                }, false)
            }
        }
        CordsLabel {
            visible: !editor.visible
            text: "Sorted by priority. Priority controls management hierarchy; it does not grant inherited permissions."
            color: Theme.secondary
            Layout.fillWidth: true
        }
        CordsLabel {
            visible: !!workspace.reorderError
            text: workspace.reorderError
            color: Theme.warning
            Layout.fillWidth: true
        }
        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 60
            model: filtered
            clip: true
            reuseItems: true
            spacing: 4
            delegate: CordsButton {
                id: role
                required property var row
                objectName: "roleRow-" + row.id
                width: list.width
                height: 62
                quiet: true
                selected: editor.visible && editor.draft.id === row.id
                text: "$" + row.name
                enabled: !workspace.desktop.busy
                contentItem: RowLayout {
                    spacing: 12
                    Item {
                        visible: !!role.row.editable && role.row.id !== "builtin:default" && !search.text.trim()
                        Layout.preferredWidth: 24
                        Layout.preferredHeight: 32
                        CordsLabel { anchors.centerIn: parent; text: "⠿"; color: Theme.secondary }
                        Item {
                            id: dragProxy
                            property string roleId: role.row.id
                            Drag.active: dragHandle.drag.active
                            Drag.source: dragProxy
                            Drag.keys: ["cords-role"]
                            Drag.supportedActions: Qt.MoveAction
                        }
                        MouseArea {
                            id: dragHandle
                            objectName: "roleDragHandle"
                            anchors.fill: parent
                            enabled: !workspace.desktop.busy && !workspace.dirty
                            cursorShape: Qt.SizeAllCursor
                            drag.target: dragProxy
                            onReleased: { dragProxy.Drag.drop(); dragProxy.x = 0; dragProxy.y = 0; }
                            onCanceled: { dragProxy.Drag.cancel(); dragProxy.x = 0; dragProxy.y = 0; }
                            Accessible.name: "Drag to reorder $" + role.row.name + ". Use Alt Up or Alt Down on the role for keyboard movement."
                        }
                    }
                    Rectangle {
                        implicitWidth: 14
                        implicitHeight: 14
                        radius: 7
                        color: role.row.color || Theme.accent
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        CordsLabel {
                            text: "$" + role.row.name
                            font.bold: true
                            Layout.fillWidth: true
                            wrapMode: Text.NoWrap
                            elide: Text.ElideRight
                        }
                        CordsLabel {
                            visible: !editor.visible
                            text: (role.row.id.startsWith("builtin:") ? "Built-in" : "Custom") + " · Priority " + role.row.priority
                            color: Theme.secondary
                            font.pixelSize: Theme.captionSize
                        }
                    }
                    CordsLabel {
                        visible: !editor.visible
                        text: workspace.desktop.state.canRoles ? "Edit  ›" : "Inspect  ›"
                        color: Theme.secondary
                    }
                }
                onClicked: editor.edit(row, true)
                Keys.onPressed: event => {
                    if ((event.modifiers & Qt.AltModifier) && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
                        workspace.moveAdjacent(row.id, event.key === Qt.Key_Up ? -1 : 1);
                        event.accepted = true;
                    }
                }
                DropArea {
                    anchors.fill: parent
                    keys: ["cords-role"]
                    onDropped: drop => {
                        workspace.moveRole(drop.source.roleId, role.row.id, drop.y < height / 2);
                        drop.acceptProposedAction();
                    }
                    Rectangle {
                        visible: parent.containsDrag
                        anchors.fill: parent
                        color: "transparent"
                        border.color: Theme.accent
                        radius: 4
                    }
                }
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: menu.popup()
                }
                CordsMenu {
                    id: menu
                    CordsMenuItem {
                        text: "Inspect role"
                        onTriggered: editor.edit(role.row, true)
                    }
                    CordsMenuItem {
                        text: "Move up"
                        enabled: !!role.row.editable && !workspace.dirty && !workspace.desktop.busy && !search.text.trim()
                        onTriggered: workspace.moveAdjacent(role.row.id, -1)
                    }
                    CordsMenuItem {
                        text: "Move down"
                        enabled: !!role.row.editable && !workspace.dirty && !workspace.desktop.busy && !search.text.trim()
                        onTriggered: workspace.moveAdjacent(role.row.id, 1)
                    }
                    CordsMenuItem {
                        text: "Copy role ID"
                        onTriggered: workspace.desktop.copy(role.row.id)
                    }
                }
            }
            ScrollBar.vertical: ScrollBar {}
            CordsLabel {
                anchors.centerIn: parent
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                visible: filtered.count === 0
                text: workspace.desktop.busy ? "Loading roles…" : "No loaded roles match. Refresh to retrieve the catalog."
                color: Theme.secondary
            }
        }
        CordsLabel {
            text: filtered.count + " roles shown. Counts include active member accounts, not devices."
            color: Theme.secondary
            Layout.fillWidth: true
        }
    }
    RoleEditor {
        id: editor
        desktop: workspace.desktop
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumWidth: 180
    }
    ConfirmDialog {
        id: reorder
        objectName: "reorderRolesConfirmation"
        title: "Apply role order?"
        explanation: "Update this editable role segment in one atomic change? Priorities determine management hierarchy. Protected roles stay fixed; the server rechecks the final policy."
        confirmText: "Apply order"
    }
    ConfirmDialog {
        id: discard
        objectName: "leaveRolesConfirmation"
        title: "Discard role edits?"
        explanation: "Your unsaved changes will be lost."
        confirmText: "Discard"
    }
}
