pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Pane {
    id: panel
    required property Controller desktop
    signal profileRequested(string account, Item anchor)
    signal restoreAccountFocus(string account)
    padding: 12
    background: Rectangle {
        color: Theme.navigation
    }
    RowFilter {
        id: filteredMembers
        sourceModel: panel.desktop.members
        query: search.text
    }
    ColumnLayout {
        anchors.fill: parent
        RowLayout {
            Layout.fillWidth: true
            CordsLabel {
                text: "MEMBERS"
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                font.bold: true
                Layout.fillWidth: true
            }
            CordsIconButton {
                objectName: "refreshMembers"
                iconName: "refresh"
                text: "Refresh members"
                enabled: !panel.desktop.busy && !!panel.desktop.state.connected
                onClicked: panel.desktop.loadDirectory("")
            }
        }
        CordsField {
            id: search
            placeholderText: "Search loaded members"
            Layout.fillWidth: true
        }
        ListView {
            id: memberList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            reuseItems: true
            model: filteredMembers
            spacing: 4
            section.property: "row.group"
            section.delegate: CordsLabel {
                required property string section
                text: section
                font.pixelSize: Theme.captionSize
                color: Theme.secondary
                topPadding: 12
                bottomPadding: 4
            }
            delegate: CordsButton {
                id: member
                objectName: "memberRow"
                required property var row
                readonly property var person: panel.desktop.state.people ? panel.desktop.state.people[row.id] || ({}) : ({})
                width: ListView.view.width
                text: row.name
                quiet: true
                height: Theme.memberRowHeight
                padding: 6
                help: "Open verified profile"
                contentItem: RowLayout {
                    spacing: 8
                    CordsAvatar {
                        name: member.row.name
                        account: member.row.id
                        source: member.person.image || ""
                        shape: member.person.shape || "circle"
                        Layout.preferredWidth: 32
                        Layout.preferredHeight: 32
                        playing: member.hovered && member.visible && member.y + member.height >= memberList.contentY && member.y <= memberList.contentY + memberList.height
                    }
                    CordsLabel {
                        text: member.row.name
                        Layout.fillWidth: true
                        wrapMode: Text.NoWrap
                        elide: Text.ElideRight
                    }
                }
                Connections {
                    target: panel
                    function onRestoreAccountFocus(account) {
                        if (member.row.id === account && member.visible)
                            member.forceActiveFocus();
                    }
                }
                onClicked: panel.profileRequested(row.id, member)
            }
            ScrollBar.vertical: ScrollBar {}
        }
        CordsLabel {
            visible: filteredMembers.count === 0
            text: search.text && panel.desktop.members.count > 0 ? "No loaded members match your search." : panel.desktop.state.ownership === "UNCLAIMED" ? "Members will appear after this server is claimed." : panel.desktop.state.admission === "pending" ? "Membership is waiting for administrator approval." : !panel.desktop.state.connected ? "Reconnect to retrieve this server's members." : panel.desktop.state.directoryLoaded ? "No admitted accounts in this page." : panel.desktop.busy ? "Loading admitted members…" : panel.desktop.state.error ? "Members could not be retrieved. Refresh to retry." : "Members have not been loaded. Refresh to retrieve them."
            color: Theme.secondary
            Layout.fillWidth: true
        }
        CordsLabel {
            text: panel.desktop.state.presentationError || ""
            visible: text.length > 0
            color: Theme.warning
            Layout.fillWidth: true
            Accessible.role: Accessible.AlertMessage
        }
        CordsButton {
            text: "Load more"
            visible: !!panel.desktop.state.directoryAfter
            enabled: !panel.desktop.busy
            Layout.fillWidth: true
            onClicked: panel.desktop.loadDirectory(panel.desktop.state.directoryAfter)
        }
    }
}
