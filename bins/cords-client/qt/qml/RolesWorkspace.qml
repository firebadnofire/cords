pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: workspace
    required property Controller desktop
    readonly property bool modalOpen: editor.visible
    spacing: 12
    RowFilter {
        id: filtered
        sourceModel: workspace.desktop.roles
        query: search.text
        sortField: "priority"
        descending: true
    }
    CordsLabel {
        text: "Roles & permissions"
        font.pixelSize: Theme.headingSize
    }
    CordsLabel {
        text: workspace.desktop.state.rolesActive ? "Server-enforced policy · revision " + workspace.desktop.state.revision : "Preparatory role catalog · saving roles does not activate channel permission enforcement."
        color: Theme.warning
        Layout.fillWidth: true
    }
    CordsButton {
        text: "Default permissions  ›"
        Layout.fillWidth: true
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
            width: Math.min(300, workspace.width)
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
            enabled: !workspace.desktop.busy && !!workspace.desktop.state.canRoles
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
        text: "Sorted by priority. Priority controls management hierarchy; it does not grant inherited permissions."
        color: Theme.secondary
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
            width: list.width
            height: 62
            quiet: true
            text: "$" + row.name
            enabled: !workspace.desktop.busy
            contentItem: RowLayout {
                spacing: 12
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
                        text: (role.row.id.startsWith("builtin:") ? "Built-in" : "Custom") + " · Priority " + role.row.priority
                        color: Theme.secondary
                        font.pixelSize: Theme.captionSize
                    }
                }
                CordsLabel {
                    text: workspace.desktop.state.canRoles ? "Edit  ›" : "Inspect  ›"
                    color: Theme.secondary
                }
            }
            onClicked: editor.edit(row, true)
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: menu.popup()
            }
            Menu {
                id: menu
                MenuItem {
                    text: "Inspect role"
                    onTriggered: editor.edit(role.row, true)
                }
                MenuItem {
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
        text: filtered.count + " roles shown. Member totals are unavailable from the role catalog."
        color: Theme.secondary
        Layout.fillWidth: true
    }
    RoleEditor {
        id: editor
        desktop: workspace.desktop
    }
}
