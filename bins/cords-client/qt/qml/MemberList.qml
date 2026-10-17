pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Pane {
    id: panel
    required property Controller desktop
    signal profileRequested(string account)
    padding: 12
    background: Rectangle {
        color: Theme.navigation
    }
    ColumnLayout {
        anchors.fill: parent
        CordsLabel {
            text: "MEMBERS"
            color: Theme.secondary
            font.pixelSize: Theme.captionSize
            font.bold: true
        }
        CordsField {
            id: search
            placeholderText: "Find a member"
            Layout.fillWidth: true
        }
        ListView {
            id: memberList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            reuseItems: true
            model: panel.desktop.members
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
                visible: search.text.length === 0 || row.name.toLowerCase().includes(search.text.toLowerCase()) || row.id.includes(search.text)
                height: visible ? Theme.rowHeight : 0
                help: "Open verified profile"
                contentItem: RowLayout {
                    spacing: 8
                    CordsAvatar {
                        name: member.row.name
                        account: member.row.id
                        source: member.person.image || ""
                        shape: member.person.shape || "circle"
                        Layout.preferredWidth: 28
                        Layout.preferredHeight: 28
                        playing: member.hovered && member.visible && member.y + member.height >= memberList.contentY && member.y <= memberList.contentY + memberList.height
                    }
                    CordsLabel {
                        text: member.row.name
                        Layout.fillWidth: true
                        wrapMode: Text.NoWrap
                        elide: Text.ElideRight
                    }
                }
                onClicked: panel.profileRequested(row.id)
            }
            ScrollBar.vertical: ScrollBar {}
        }
        CordsLabel {
            visible: panel.desktop.members.count === 0
            text: panel.desktop.state.directoryLoaded ? "No admitted accounts in this page." : "Loading admitted members…"
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
            text: "Refresh members"
            enabled: !panel.desktop.busy
            Layout.fillWidth: true
            onClicked: panel.desktop.loadDirectory("")
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
