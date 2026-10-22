pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: editor
    objectName: "roleEditor"
    required property Controller desktop
    property var draft: ({})
    property var reviewedRevision: 0
    property string draftBaseline: ""
    property string lastPayload: ""
    property string requestId: ""
    property bool submitting: false
    property bool existing: false
    property string serverId: ""
    property string localAccount: ""
    readonly property bool dirty: JSON.stringify(draft) !== draftBaseline
    property int rolePresentationRevision: 0
    Connections {
        target: editor.desktop.roles
        function onChanged() { ++editor.rolePresentationRevision; }
    }
    readonly property var currentRole: {
        const changed = rolePresentationRevision;
        const revision = desktop.state.revision;
        const count = desktop.roles.count;
        const index = desktop.roles.indexOf(draft.id || "");
        return index >= 0 ? desktop.roles.get(index) : ({});
    }
    readonly property bool canDelete: existing && !!currentRole.deletable && canEdit
    readonly property bool canEdit: visible && localAccount === (desktop.state.account || "") && serverId === (desktop.state.server || "") && (existing ? !!currentRole.editable : !!desktop.state.canCreateRoles) && !desktop.busy
    readonly property bool valid: !!draft.name && draft.name.trim().length > 0 && /^#[0-9a-fA-F]{6}$/.test(draft.color || "")
    readonly property string title: existing ? "Edit $" + (draft.name || "role") : "Create role"
    width: Math.min(680, parent ? parent.width - 32 : 680)
    height: Math.min(760, parent ? parent.height - 32 : 760)
    visible: false
    property int tab: 0
    readonly property bool modalOpen: discard.visible || deletion.visible || roleMembers.modalOpen
    function close() { visible = false; }
    function open() { visible = true; }
    onVisibleChanged: if (!visible) {
        discard.close();
        deletion.close();
        discard.operation = null;
        deletion.operation = null;
        submitting = false;
        draft = ({});
        draftBaseline = JSON.stringify(draft);
        lastPayload = "";
        requestId = "";
    }
    function edit(role, isExisting) {
        if (visible && dirty) {
            const next = Object.assign({}, role);
            discard.operation = function () { editor.beginEdit(next, isExisting); };
            discard.open();
            return;
        }
        beginEdit(role, isExisting);
    }
    function beginEdit(role, isExisting) {
        serverId = desktop.state.server || "";
        localAccount = desktop.state.account || "";
        tab = 0;
        permissionSearch.clear();
        draft = Object.assign({}, role);
        existing = isExisting;
        reviewedRevision = desktop.state.revision;
        draftBaseline = JSON.stringify(draft);
        lastPayload = "";
        submitting = false;
        discard.operation = function () { editor.close(); };
        deletion.operation = editor.removeRole;
        open();
    }
    function change(key, value) {
        let next = Object.assign({}, draft);
        next[key] = value;
        draft = next;
    }
    function togglePermission(id, checked) {
        let permissions = (draft.permissions || []).filter(value => value !== id);
        if (checked)
            permissions.push(id);
        change("permissions", permissions.sort());
    }
    function requestClose() {
        if (desktop.busy)
            return;
        if (!dirty) {
            close();
            return;
        }
        discard.operation = function () { editor.close(); };
        discard.open();
    }
    function save() {
        if (!canEdit || !valid || reviewedRevision !== desktop.state.revision)
            return;
        const payload = JSON.stringify(draft);
        if (payload !== lastPayload) {
            lastPayload = payload;
            requestId = desktop.newId();
        }
        submitting = true;
        desktop.putRole(draft, reviewedRevision, requestId);
    }
    function removeRole() {
        if (!canDelete || !existing || (draft.id || "").startsWith("builtin:") || reviewedRevision !== desktop.state.revision)
            return;
        const payload = JSON.stringify(["delete", draft.id, reviewedRevision]);
        if (payload !== lastPayload) {
            lastPayload = payload;
            requestId = desktop.newId();
        }
        submitting = true;
        desktop.deleteRole(draft.id, reviewedRevision, requestId);
    }
    Connections {
        target: editor.desktop
        function onStateChanged() {
            if (editor.visible && (editor.serverId !== (editor.desktop.state.server || "") || editor.localAccount !== (editor.desktop.state.account || ""))) {
                editor.close();
                return;
            }
            if (!editor.submitting || editor.desktop.busy)
                return;
            editor.submitting = false;
            if (!editor.desktop.state.error && editor.desktop.state.notice === "Role policy saved")
                editor.close();
        }
    }
    Shortcut {
        sequence: "Escape"
        enabled: editor.visible && !discard.visible && !deletion.visible
        onActivated: editor.requestClose()
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 12
        CordsLabel {
            text: editor.title
            font.pixelSize: Theme.headingSize
            Layout.fillWidth: true
        }
        RowLayout {
            Layout.fillWidth: true
            Repeater {
                model: ["Display", "Permissions", "Manage Members"]
                CordsButton {
                    required property int index
                    required property string modelData
                    objectName: "roleTab-" + modelData
                    text: modelData
                    quiet: true
                    selected: editor.tab === index
                    onClicked: editor.tab = index
                }
            }
        }
        CordsField {
            id: permissionSearch
            objectName: "permissionSearch"
            visible: editor.tab === 1
            placeholderText: "Search permissions"
            Accessible.name: placeholderText
            Layout.fillWidth: true
        }
        CordsLabel {
            text: editor.desktop.state.rolesActive ? "The server validates every grant and hierarchy change." : "Preparatory catalog: saving does not activate permission enforcement."
            color: Theme.warning
            Layout.fillWidth: true
        }
        CordsLabel {
            text: editor.desktop.state.error || ""
            visible: !!text
            color: Theme.danger
            Layout.fillWidth: true
        }
        CordsLabel {
            text: "Policy changed since this editor opened. Your draft is preserved; cancel and refresh before applying it."
            visible: editor.reviewedRevision !== editor.desktop.state.revision
            color: Theme.warning
            Layout.fillWidth: true
        }
        ScrollView {
            id: scroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            ColumnLayout {
                width: scroll.availableWidth
                spacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    visible: editor.tab === 0
                    enabled: editor.canEdit
                    CordsLabel {
                        text: "Name"
                        font.bold: true
                    }
                    CordsField {
                        objectName: "roleName"
                        text: editor.draft.name || ""
                        maximumLength: 64
                        readOnly: (editor.draft.id || "").startsWith("builtin:")
                        Layout.fillWidth: true
                        Accessible.name: "Role name"
                        onTextEdited: editor.change("name", text)
                    }
                    CordsLabel {
                        text: "Color · #RRGGBB"
                        font.bold: true
                    }
                    RowLayout {
                        Rectangle {
                            implicitWidth: 24
                            implicitHeight: 24
                            radius: 12
                            color: /^#[0-9a-fA-F]{6}$/.test(editor.draft.color || "") ? editor.draft.color : Theme.muted
                        }
                        CordsField {
                            objectName: "roleColor"
                            text: editor.draft.color || ""
                            maximumLength: 7
                            Layout.fillWidth: true
                            Accessible.name: "Role color"
                            onTextEdited: editor.change("color", text)
                        }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6
                        Repeater {
                            model: ["#83bdd9", "#78c99a", "#e8bc70", "#ee777d", "#ba9de8", "#c6c9d0"]
                            CordsButton {
                                id: swatch
                                required property string modelData
                                text: modelData
                                Accessible.name: "Use role color " + modelData
                                implicitWidth: 34
                                implicitHeight: 30
                                selected: editor.draft.color === modelData
                                contentItem: Rectangle { color: swatch.modelData; radius: 4; implicitWidth: 18; implicitHeight: 18 }
                                onClicked: editor.change("color", modelData)
                            }
                        }
                    }
                    CordsLabel {
                        text: "Preview"
                        font.bold: true
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: preview.implicitHeight + 24
                        color: Theme.conversation
                        radius: Theme.radius
                        ColumnLayout {
                            id: preview
                            anchors.fill: parent
                            anchors.margins: 12
                            CordsRoleChip { roleName: editor.draft.name || "New role"; roleColor: editor.draft.color || ""; Layout.maximumWidth: preview.width }
                            CordsLabel { text: "Sample member"; font.bold: true; Layout.fillWidth: true }
                            CordsLabel { text: "A message in the conversation."; Layout.fillWidth: true; color: Theme.secondary }
                        }
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: elevatedPreview.implicitHeight + 24
                        color: Theme.elevated
                        radius: Theme.radius
                        ColumnLayout {
                            id: elevatedPreview
                            anchors.fill: parent
                            anchors.margins: 12
                            CordsRoleChip { roleName: editor.draft.name || "New role"; roleColor: editor.draft.color || ""; Layout.maximumWidth: elevatedPreview.width }
                            CordsLabel { text: "Sample member"; font.bold: true; Layout.fillWidth: true }
                            CordsLabel { text: "A message on an elevated surface."; Layout.fillWidth: true; color: Theme.secondary }
                        }
                    }
                    CordsLabel {
                        text: "Description"
                        font.bold: true
                    }
                    CordsField {
                        objectName: "roleDescription"
                        text: editor.draft.description || ""
                        maximumLength: 1000
                        Layout.fillWidth: true
                        Accessible.name: "Role description"
                        onTextEdited: editor.change("description", text)
                    }
                    RowLayout {
                        CordsLabel {
                            text: "Priority"
                        }
                        SpinBox {
                            objectName: "rolePriority"
                            from: 0
                            to: 2147483647
                            value: editor.draft.priority || 0
                            enabled: editor.draft.id !== "builtin:default"
                            palette.text: Theme.text
                            palette.base: Theme.rail
                            palette.button: Theme.elevated
                            palette.buttonText: Theme.text
                            editable: true
                            Accessible.name: "Role priority"
                            onValueModified: editor.change("priority", value)
                        }
                    }
                    CordsLabel {
                        text: "Higher priority controls management hierarchy; it does not inherit lower roles. Admin explicitly inherits moderator. Owner authority is separate."
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                    CordsCheck {
                        objectName: "roleMentionable"
                        text: "Role mentions (unavailable)"
                        enabled: false
                        checked: !!editor.draft.mentionable
                    }
                    CordsLabel {
                        text: "Structured role mentions are not implemented. The saved mentionability value is preserved."
                        Layout.fillWidth: true
                        color: Theme.secondary
                    }
                }
                PermissionEditor {
                    objectName: "rolePermissions"
                    visible: editor.tab === 1
                    enabled: editor.canEdit
                    query: permissionSearch.text
                    Layout.fillWidth: true
                    permissions: editor.draft.permissions || []
                    defaultRole: editor.draft.id === "builtin:default"
                    onPermissionToggled: (id, checked) => editor.togglePermission(id, checked)
                }
                RoleMembersWorkspace {
                    id: roleMembers
                    desktop: editor.desktop
                    roleId: editor.draft.id || ""
                    saved: editor.existing
                    blocked: editor.dirty
                    visible: editor.tab === 2 && editor.visible
                    Layout.fillWidth: true
                    onMembershipChanged: revision => { if (!editor.dirty) editor.reviewedRevision = revision; }
                }
            }
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            CordsButton {
                objectName: "saveRole"
                text: editor.submitting ? "Saving…" : "Save role"
                primary: true
                enabled: editor.canEdit && editor.valid && (editor.dirty || !editor.existing) && editor.reviewedRevision === editor.desktop.state.revision
                onClicked: editor.save()
            }
            CordsButton {
                objectName: "cancelRoleEditor"
                text: "Back to roles"
                enabled: !editor.desktop.busy
                onClicked: editor.requestClose()
            }
            CordsButton {
                objectName: "deleteRole"
                text: "Delete role…"
                destructive: true
                visible: editor.existing && !(editor.draft.id || "").startsWith("builtin:")
                enabled: editor.canDelete && editor.reviewedRevision === editor.desktop.state.revision
                onClicked: deletion.open()
            }
        }
    }
    ConfirmDialog {
        id: discard
        objectName: "discardRoleConfirmation"
        title: "Discard role edits?"
        explanation: "Your unsaved changes will be lost."
        confirmText: "Discard"
    }
    ConfirmDialog {
        id: deletion
        objectName: "deleteRoleConfirmation"
        title: "Delete role?"
        explanation: "Delete $" + (editor.draft.name || "") + " and remove its assignments? The server validates your current authority and policy revision."
        confirmText: "Delete role"
    }
}
