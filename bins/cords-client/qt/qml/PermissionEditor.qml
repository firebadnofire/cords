pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: editor
    property var permissions: []
    property bool defaultRole: false
    property string query: ""
    readonly property var unavailable: ["server.invite.create", "server.invite.manage", "server.member.kick", "server.member.ban", "server.member.timeout", "server.moderation.read", "channel.messages.manage", "channel.emojis.manage", "mention_roles", "server.roles.mentionability"]
    readonly property var matching: capabilities.filter(value => !query.trim() || (value.group + " " + value.label + " " + value.description + " " + value.id).toLowerCase().includes(query.trim().toLowerCase()))
    signal permissionToggled(string id, bool checked)
    spacing: 8
    readonly property var capabilities: [
        {
            "group": "Server",
            "id": "server.read",
            "label": "View server",
            "description": "Discover server metadata.",
            "safe": true
        },
        {
            "group": "Server",
            "id": "server.manage",
            "label": "Manage server",
            "description": "Server configuration authority; available operations depend on server support.",
            "safe": false
        },
        {
            "group": "Server",
            "id": "server.roles.manage",
            "label": "Manage roles",
            "description": "Create and edit roles below your authority.",
            "safe": false
        },
        {
            "group": "Server",
            "id": "server.roles.assign",
            "label": "Assign roles",
            "description": "Assign and remove roles below your authority.",
            "safe": false
        },
        {
            "group": "Server",
            "id": "server.audit.read",
            "label": "Read audit records",
            "description": "Read administrative policy changes.",
            "safe": false
        },
        {
            "group": "Admission",
            "id": "server.member.approve",
            "label": "Approve accounts",
            "description": "Review account admission requests.",
            "safe": false
        },
        {
            "group": "Admission",
            "id": "server.device.approve",
            "label": "Approve devices",
            "description": "Review device admission independently of account trust.",
            "safe": false
        },
        {
            "group": "Admission",
            "id": "server.invite.create",
            "label": "Create invites",
            "description": "Reserved invite capability; invite operations are not available in this client.",
            "safe": false
        },
        {
            "group": "Admission",
            "id": "server.invite.manage",
            "label": "Manage invites",
            "description": "Reserved invite management capability.",
            "safe": false
        },
        {
            "group": "Channels",
            "id": "channel.view",
            "label": "View channels",
            "description": "Discover permitted channel metadata.",
            "safe": true
        },
        {
            "group": "Channels",
            "id": "channel.read",
            "label": "Read messages",
            "description": "Receive permitted events; encrypted content also requires MLS membership.",
            "safe": true
        },
        {
            "group": "Channels",
            "id": "channel.write",
            "label": "Send messages",
            "description": "Send events in permitted channels.",
            "safe": true
        },
        {
            "group": "Channels",
            "id": "channel.create",
            "label": "Create channels",
            "description": "Create new channel identities.",
            "safe": false
        },
        {
            "group": "Channels",
            "id": "channel.manage",
            "label": "Manage channels",
            "description": "Manage permitted channel configuration.",
            "safe": false
        },
        {
            "group": "Channels",
            "id": "channel.retire",
            "label": "Retire channels",
            "description": "Retire permitted channels; history remains separate.",
            "safe": false
        },
        {
            "group": "Channels",
            "id": "channel.permissions.manage",
            "label": "Manage channel permissions",
            "description": "Manage resource permission overrides where supported.",
            "safe": false
        },
        {
            "group": "Channels",
            "id": "channel.members.manage",
            "label": "Manage channel members",
            "description": "Manage permitted channel rosters; device trust remains required.",
            "safe": false
        },
        {
            "group": "Encryption",
            "id": "channel.mls.commit",
            "label": "Commit MLS changes",
            "description": "Coordinate validated cryptographic changes; never grants hidden membership.",
            "safe": true
        },
        {
            "group": "Encryption",
            "id": "keypackage.publish",
            "label": "Publish KeyPackages",
            "description": "Publish this device\u2019s validated MLS KeyPackages.",
            "safe": true
        },
        {
            "group": "Moderation",
            "id": "server.member.kick",
            "label": "Kick members",
            "description": "Reserved moderation capability; no kick action is exposed by this client.",
            "safe": false
        },
        {
            "group": "Moderation",
            "id": "server.member.ban",
            "label": "Ban members",
            "description": "Reserved moderation capability; no ban action is exposed by this client.",
            "safe": false
        },
        {
            "group": "Moderation",
            "id": "server.member.timeout",
            "label": "Timeout members",
            "description": "Reserved moderation capability.",
            "safe": false
        },
        {
            "group": "Moderation",
            "id": "server.moderation.read",
            "label": "Read moderation records",
            "description": "Reserved moderation history capability.",
            "safe": false
        },
        {
            "group": "Moderation",
            "id": "channel.messages.manage",
            "label": "Manage messages",
            "description": "Reserved message moderation capability.",
            "safe": false
        },
        {
            "group": "Expression",
            "id": "channel.emojis.manage",
            "label": "Manage emojis",
            "description": "Reserved channel emoji capability.",
            "safe": false
        },
        {
            "group": "Expression",
            "id": "mention_roles",
            "label": "Mention roles",
            "description": "Role mention capability; structured mention support is required.",
            "safe": true
        },
        {
            "group": "Expression",
            "id": "server.roles.mentionability",
            "label": "Manage mentionability",
            "description": "Control whether roles may be mentioned.",
            "safe": false
        }
    ]
    CordsLabel {
        visible: editor.matching.length === 0
        text: "No permissions match your search."
        color: Theme.secondary
        Layout.fillWidth: true
    }
    Repeater {
        model: editor.matching
        delegate: ColumnLayout {
            id: permission
            required property var modelData
            required property int index
            Layout.fillWidth: true
            CordsLabel {
                visible: permission.index === 0 || editor.matching[permission.index - 1].group !== permission.modelData.group
                text: permission.modelData.group
                font.bold: true
                font.pixelSize: 17
                Layout.topMargin: 16
            }
            RowLayout {
                Layout.fillWidth: true
                CordsLabel {
                    text: permission.modelData.label
                    Layout.fillWidth: true
                }
                CordsCheck {
                    objectName: "rolePermission-" + permission.modelData.id
                    Accessible.name: permission.modelData.label
                    checked: editor.permissions.includes(permission.modelData.id)
                    enabled: !editor.unavailable.includes(permission.modelData.id) && (!editor.defaultRole || permission.modelData.safe)
                    onToggled: editor.permissionToggled(permission.modelData.id, checked)
                }
            }
            CordsLabel {
                text: permission.modelData.description + (editor.unavailable.includes(permission.modelData.id) ? " Unavailable: the corresponding operation is not implemented. Existing values are preserved." : editor.defaultRole && !permission.modelData.safe ? " Not allowed on the default role." : "")
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                Layout.fillWidth: true
            }
        }
    }
}
