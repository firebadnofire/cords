pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: workspace
    required property Controller desktop
    readonly property bool modalOpen: assignments.visible
    signal profileRequested(string account)
    property string selectedAccount: ""
    property var selectedMember: ({})
    readonly property string scope: JSON.stringify([desktop.state.account || "", desktop.state.server || ""])
    onScopeChanged: {
        selectedAccount = "";
        selectedMember = ({});
        search.text = "";
        roleChoice.currentIndex = 0;
        assignments.close();
    }
    function selectMember(member) {
        selectedAccount = member.id;
        selectedMember = member;
    }
    spacing: 12
    RowFilter {
        id: filtered
        sourceModel: workspace.desktop.members
        query: search.text
        roleFilter: roleChoice.currentIndex > 0 ? workspace.desktop.roles.get(roleChoice.currentIndex - 1).id || "" : ""
    }
    CordsLabel {
        text: "Members"
        font.pixelSize: Theme.headingSize
    }
    CordsLabel {
        text: "Admitted accounts on this server. Profile names are presentation labels; fingerprints identify accounts."
        color: Theme.secondary
        Layout.fillWidth: true
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        CordsField {
            id: search
            objectName: "memberSearch"
            width: Math.min(320, workspace.width)
            placeholderText: "Search loaded members by name or ID"
            Accessible.name: placeholderText
        }
        CordsButton {
            text: "Refresh"
            enabled: !workspace.desktop.busy
            onClicked: workspace.desktop.loadDirectory("")
        }
        CordsButton {
            text: "Load more"
            visible: !!workspace.desktop.state.directoryAfter
            enabled: !workspace.desktop.busy
            onClicked: workspace.desktop.loadDirectory(workspace.desktop.state.directoryAfter)
        }
    }
    CordsComboBox {
        id: roleChoice
        Layout.fillWidth: true
        Layout.maximumWidth: 320
        model: {
            const revision = workspace.desktop.state.revision;
            let values = ["All roles"];
            for (let i = 0; i < workspace.desktop.roles.count; ++i)
                values.push("$" + workspace.desktop.roles.get(i).name);
            return values;
        }
        Accessible.name: "Filter members by role"
    }
    RowLayout {
        Layout.fillWidth: true
        CordsButton {
            text: "Name " + (filtered.sortField === "name" ? (filtered.descending ? "↓" : "↑") : "")
            quiet: true
            Layout.fillWidth: true
            onClicked: {
                filtered.descending = filtered.sortField === "name" && !filtered.descending;
                filtered.sortField = "name";
            }
        }
        CordsButton {
            text: "Account ID " + (filtered.sortField === "id" ? (filtered.descending ? "↓" : "↑") : "")
            quiet: true
            Layout.fillWidth: true
            onClicked: {
                filtered.descending = filtered.sortField === "id" && !filtered.descending;
                filtered.sortField = "id";
            }
        }
    }
    ListView {
        id: list
        objectName: "administrativeMembers"
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumHeight: 120
        model: filtered
        clip: true
        reuseItems: true
        spacing: 2
        keyNavigationEnabled: true
        activeFocusOnTab: true
        Keys.onReturnPressed: if (currentIndex >= 0)
            workspace.selectMember(filtered.get(currentIndex))
        delegate: CordsButton {
            id: member
            required property var row
            required property int index
            objectName: "administrativeMemberRow"
            width: list.width
            height: 64
            quiet: true
            selected: workspace.selectedAccount === row.id
            text: row.name
            contentItem: RowLayout {
                spacing: 12
                CordsAvatar {
                    name: member.row.name
                    account: member.row.id
                    source: (workspace.desktop.state.people || ({}))[account]?.image || ""
                    shape: (workspace.desktop.state.people || ({}))[account]?.shape || "circle"
                    playing: member.hovered
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    CordsLabel {
                        text: member.row.name
                        font.bold: true
                        Layout.fillWidth: true
                        wrapMode: Text.NoWrap
                        elide: Text.ElideRight
                    }
                    CordsLabel {
                        text: (member.row.roles || []).join(", ") || "No explicit roles in loaded data"
                        color: Theme.secondary
                        font.pixelSize: Theme.captionSize
                        Layout.fillWidth: true
                        wrapMode: Text.NoWrap
                        elide: Text.ElideRight
                    }
                }
                CordsLabel {
                    text: member.row.id
                    Layout.preferredWidth: list.width * 0.42
                    wrapMode: Text.NoWrap
                    elide: Text.ElideMiddle
                    color: Theme.secondary
                }
            }
            onClicked: {
                list.currentIndex = index;
                workspace.selectMember(row);
            }
            onDoubleClicked: workspace.profileRequested(row.id)
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: {
                    workspace.selectMember(member.row);
                    actions.popup();
                }
            }
            CordsMenu {
                id: actions
                CordsMenuItem {
                    text: "View profile"
                    onTriggered: workspace.profileRequested(member.row.id)
                }
                CordsMenuItem {
                    text: "Copy account ID"
                    onTriggered: workspace.desktop.copy(member.row.id)
                }
                CordsMenuItem {
                    text: "Read role assignments"
                    enabled: !workspace.desktop.busy
                    onTriggered: workspace.desktop.loadAccountRoles(member.row.id)
                }
            }
        }
        ScrollBar.vertical: ScrollBar {}
        CordsLabel {
            anchors.centerIn: parent
            width: Math.max(100, parent.width - 32)
            horizontalAlignment: Text.AlignHCenter
            visible: filtered.count === 0
            color: Theme.secondary
            text: workspace.desktop.busy ? "Loading members…" : workspace.desktop.state.directoryLoaded ? "No loaded members match these filters." : "Refresh to load the member directory."
        }
    }
    CordsLabel {
        text: filtered.count + " shown · " + workspace.desktop.members.count + " loaded" + (workspace.desktop.state.directoryAfter ? " · More available" : "")
        color: Theme.secondary
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        visible: !!workspace.selectedAccount
        CordsButton {
            text: "View profile"
            enabled: !workspace.desktop.busy
            onClicked: workspace.profileRequested(workspace.selectedAccount)
        }
        CordsButton {
            text: "Copy account ID"
            onClicked: workspace.desktop.copy(workspace.selectedAccount)
        }
        CordsButton {
            text: "Manage roles…"
            enabled: !workspace.desktop.busy
            onClicked: assignments.inspect(workspace.selectedMember)
        }
    }
    MemberRolesDialog {
        id: assignments
        desktop: workspace.desktop
    }
}
