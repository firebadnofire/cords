pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: shell
    required property Controller desktop
    property bool showMembers: desktop.state.preferences ? desktop.state.preferences.showMembers : true
    property bool showInfo: false
    property bool showSettings: false
    property bool showAdmin: false
    property var channel: ({})
    property string search: ""
    property string submittedBody: ""
    property string submittedRoute: ""
    readonly property bool wide: width >= 1120
    readonly property string route: desktop.state.route || ""
    onWideChanged: {
        memberDrawer.close();
        infoDrawer.close();
    }
    function openProfile(account) {
        profile.account = account;
        profile.open();
    }
    function confirm(title, explanation, action) {
        confirmation.title = title;
        confirmation.explanation = explanation;
        confirmation.operation = action;
        confirmation.open();
    }
    function selectCurrentChannel() {
        let selected = ({});
        for (let i = 0; i < desktop.channels.count; ++i) {
            const candidate = desktop.channels.get(i);
            if (candidate.id === route) {
                selected = candidate;
                break;
            }
        }
        channel = selected;
    }
    Connections {
        target: shell.desktop
        function onStateChanged() {
            shell.selectCurrentChannel();
            if (shell.submittedBody && !shell.desktop.busy) {
                if (shell.desktop.state.notice === "Message sent" && composer.text === shell.submittedBody && shell.route === shell.submittedRoute)
                    composer.clear();
                shell.submittedBody = "";
            }
        }
        function onNotificationsRequested() {
            activity.open();
        }
        function onConversationRequested() {
            shell.showSettings = false;
            shell.showAdmin = false;
            memberDrawer.close();
            infoDrawer.close();
        }
    }
    RowLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.preferredWidth: 72
            Layout.fillHeight: true
            color: Theme.rail
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 10
                spacing: 10
                CordsButton {
                    text: "C"
                    Layout.fillWidth: true
                    help: "Account settings"
                    onClicked: {
                        shell.showSettings = true;
                        shell.showAdmin = false;
                    }
                }
                ListView {
                    id: serverList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: 10
                    clip: true
                    model: shell.desktop.servers
                    delegate: CordsButton {
                        id: serverButton
                        required property var row
                        width: serverList.width
                        height: 48
                        text: row.origin.replace("https://", "").slice(0, 2).toUpperCase()
                        primary: row.selected
                        background: Rectangle {
                            radius: serverButton.row.selected || serverButton.hovered ? 13 : width / 2
                            color: serverButton.row.selected ? Theme.accent : serverButton.hovered ? Theme.hover : Theme.elevated
                            border.width: serverButton.visualFocus ? 2 : 0
                            border.color: Theme.focus
                            Behavior on radius { NumberAnimation { duration: Theme.reducedMotion ? 0 : 120 } }
                        }
                        Rectangle {
                            anchors.left: parent.left
                            anchors.leftMargin: -8
                            anchors.verticalCenter: parent.verticalCenter
                            width: 3
                            height: serverButton.row.selected ? 32 : 16
                            radius: 2
                            visible: serverButton.row.selected || serverButton.hovered
                            color: Theme.text
                        }
                        help: row.origin + (row.archived ? " (archived)" : row.error ? " · " + row.error : row.connected ? " · Connected" : " · Connecting")
                        Rectangle {
                            visible: serverButton.row.unread
                            width: 7
                            height: 7
                            radius: 4
                            color: Theme.text
                            anchors.left: parent.left
                            anchors.top: parent.top
                        }
                        enabled: !shell.desktop.busy && !row.archived
                        onClicked: {
                            shell.showSettings = false;
                            shell.showAdmin = false;
                            shell.desktop.selectServer(row.id);
                        }
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            onTapped: serverMenu.popup()
                        }
                        Menu {
                            id: serverMenu
                            MenuItem {
                                text: "Copy server origin"
                                onTriggered: shell.desktop.copy(serverButton.row.origin)
                            }
                            MenuItem {
                                text: "Server settings"
                                onTriggered: {
                                    shell.desktop.selectServer(serverButton.row.id);
                                    shell.showAdmin = true;
                                    shell.showSettings = false;
                                }
                            }
                            MenuItem {
                                text: shell.desktop.state.serverMuted ? "Unmute server notifications" : "Mute server notifications"
                                enabled: serverButton.row.selected && !shell.desktop.busy
                                onTriggered: shell.desktop.muteServer(!shell.desktop.state.serverMuted)
                            }
                            MenuItem {
                                text: "Remove locally…"
                                onTriggered: shell.confirm("Remove server locally", "Remove " + serverButton.row.origin + " from this account's local state. This does not prove remote departure.", function () {
                                    shell.desktop.removeServer(serverButton.row.id, true);
                                })
                            }
                            MenuItem {
                                text: "Verified departure…"
                                onTriggered: shell.confirm("Leave server", "Send a signed departure to " + serverButton.row.origin + " and remove local server state after verification.", function () {
                                    shell.desktop.removeServer(serverButton.row.id, false);
                                })
                            }
                            MenuItem {
                                text: "Archive…"
                                onTriggered: shell.confirm("Archive server", "Leave " + serverButton.row.origin + " and keep a sealed local archive.", function () {
                                    shell.desktop.archiveServer(serverButton.row.id);
                                })
                            }
                        }
                    }
                    ScrollBar.vertical: ScrollBar {}
                }
                CordsButton {
                    text: "+"
                    Layout.fillWidth: true
                    help: "Connect to a Cords server"
                    enabled: !shell.desktop.busy
                    onClicked: joinServer.open()
                }
                CordsButton {
                    text: "⇄"
                    Layout.fillWidth: true
                    help: "Switch identity and lock this account (Ctrl+L)"
                    onClicked: shell.desktop.lockAccount(false)
                }
            }
        }
        Rectangle {
            Layout.preferredWidth: shell.width < 850 ? 176 : shell.width >= 1400 ? 280 : 232
            Layout.fillHeight: true
            color: Theme.navigation
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 10
                CordsButton {
                    text: shell.desktop.state.origin ? shell.desktop.state.origin.replace("https://", "") : "Choose a server"
                    Layout.fillWidth: true
                    help: shell.desktop.state.origin || "Connect using + in the server rail"
                    onClicked: {
                        shell.showAdmin = !shell.showAdmin;
                        shell.showSettings = false;
                    }
                }
                CordsLabel {
                    text: shell.desktop.state.admission || "No server selected"
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                }
                CordsButton {
                    text: "Authenticate / request admission"
                    visible: !!shell.desktop.state.server && !shell.desktop.state.connected
                    Layout.fillWidth: true
                    enabled: !shell.desktop.busy
                    onClicked: shell.desktop.authenticate()
                }
                CordsField {
                    id: filter
                    placeholderText: "Find a channel"
                    Layout.fillWidth: true
                }
                RowLayout {
                    Layout.fillWidth: true
                    CordsLabel {
                        text: "TEXT CHANNELS"
                        font.pixelSize: Theme.captionSize
                        color: Theme.secondary
                        Layout.fillWidth: true
                    }
                    CordsButton {
                        text: "+"
                        implicitWidth: 32
                        visible: !!shell.desktop.state.canCreate
                        enabled: !shell.desktop.busy
                        help: "Create channel"
                        onClicked: createChannel.open()
                    }
                }
                ListView {
                    id: channelList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    model: shell.desktop.channels
                    reuseItems: true
                    clip: true
                    spacing: 4
                    delegate: CordsButton {
                        required property var row
                        width: channelList.width
                        text: "# " + row.name + (row.retired ? " · archive" : "")
                        visible: filter.text.length === 0 || row.name.toLowerCase().includes(filter.text.toLowerCase())
                        height: visible ? Theme.rowHeight : 0
                        quiet: true
                        selected: row.id === shell.route
                        enabled: !shell.desktop.busy
                        help: row.mode === "public" ? "Public: server can read messages" : "End-to-end encrypted"
                        onClicked: {
                            shell.showSettings = false;
                            shell.showAdmin = false;
                            shell.desktop.selectChannel(row.id);
                        }
                    }
                    ScrollBar.vertical: ScrollBar {}
                }
                CordsButton {
                    text: "Synchronize"
                    Layout.fillWidth: true
                    enabled: !shell.desktop.busy && !!shell.desktop.state.server
                    onClicked: shell.desktop.synchronize()
                }
                CordsButton {
                    text: shell.desktop.state.preferences ? shell.desktop.state.preferences.name : "Your account"
                    Layout.fillWidth: true
                    help: "Account settings"
                    onClicked: {
                        shell.showSettings = true;
                        shell.showAdmin = false;
                    }
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 50
                color: Theme.window
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 10
                    CordsLabel {
                        text: shell.showSettings ? "Account settings" : shell.showAdmin ? "Server settings" : shell.channel.name ? "# " + shell.channel.name : "Cords"
                        font.bold: true
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        wrapMode: Text.NoWrap
                    }
                    CordsField {
                        id: messageSearch
                        placeholderText: "Search loaded messages"
                        Layout.preferredWidth: shell.width < 850 ? 120 : 210
                        visible: shell.route.length > 0 && !shell.showSettings && !shell.showAdmin
                    }
                    CordsButton {
                        text: "Activity"
                        visible: !shell.showSettings && !shell.showAdmin && shell.desktop.notifications.count > 0
                        help: "Open pending message notifications"
                        onClicked: activity.open()
                    }
                    CordsButton {
                        text: "Members"
                        visible: !shell.showSettings && !shell.showAdmin
                        help: "Toggle public member directory"
                        onClicked: {
                            if (shell.wide)
                                shell.showMembers = !shell.showMembers;
                            else
                                memberDrawer.open();
                            shell.showInfo = false;
                            if ((shell.showMembers || !shell.wide) && !shell.desktop.busy)
                                shell.desktop.loadDirectory("");
                        }
                    }
                    CordsButton {
                        text: "Info"
                        visible: !shell.showSettings && !shell.showAdmin
                        help: "Conversation security and technical details"
                        onClicked: {
                            if (shell.wide) {
                                shell.showInfo = !shell.showInfo;
                                shell.showMembers = false;
                            } else
                                infoDrawer.open();
                        }
                    }
                    CordsButton {
                        text: "Back to chat"
                        visible: shell.showSettings || shell.showAdmin
                        onClicked: {
                            shell.showSettings = false;
                            shell.showAdmin = false;
                        }
                    }
                }
            }
            ProgressBar {
                Layout.fillWidth: true
                visible: shell.desktop.busy
                indeterminate: true
                implicitHeight: 3
            }
            CordsLabel {
                text: shell.desktop.state.error || ""
                color: Theme.danger
                visible: text.length > 0
                Layout.fillWidth: true
                padding: 10
                Accessible.role: Accessible.AlertMessage
            }
            CordsLabel {
                text: shell.desktop.state.notice || ""
                color: Theme.secondary
                visible: text.length > 0
                Layout.fillWidth: true
                padding: 8
            }
            SettingsPane {
                visible: shell.showSettings
                desktop: shell.desktop
                Layout.fillWidth: true
                Layout.fillHeight: true
                onProfileRequested: shell.openProfile(shell.desktop.state.account)
            }
            AdminPane {
                visible: shell.showAdmin
                desktop: shell.desktop
                route: shell.route
                Layout.fillWidth: true
                Layout.fillHeight: true
                onProfileRequested: account => shell.openProfile(account)
            }
            RowLayout {
                visible: !shell.showSettings && !shell.showAdmin
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: 8
                    Flow {
                        Layout.fillWidth: true
                        Layout.leftMargin: 14
                        Layout.rightMargin: 14
                        visible: !!shell.route
                        spacing: 8
                        CordsButton {
                            text: shell.desktop.state.routeMuted ? "Unmute this conversation" : "Mute this conversation"
                            enabled: !shell.desktop.busy
                            onClicked: shell.desktop.muteRoute(shell.route, !shell.desktop.state.routeMuted)
                        }
                        CordsButton {
                            text: "Older messages"
                            objectName: "olderMessages"
                            visible: !!shell.desktop.state.historyOlder
                            enabled: !shell.desktop.busy
                            onClicked: {
                                messages.followTail = false;
                                shell.desktop.olderMessages(shell.route);
                            }
                        }
                        CordsButton {
                            text: shell.desktop.state.historyNewer ? "Newer messages · Latest" : "Latest"
                            objectName: "latestMessages"
                            visible: !!shell.desktop.state.historyBrowsing
                            enabled: !shell.desktop.busy
                            onClicked: {
                                messages.followTail = true;
                                shell.desktop.latestMessages(shell.route);
                            }
                        }
                    }
                    HistoryView {
                        id: messages
                        objectName: "messageList"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        model: shell.desktop.messages
                        clip: true
                        reuseItems: true
                        spacing: Theme.compact ? 2 : 8
                        leftMargin: 14
                        rightMargin: 14
                        historyScope: [shell.desktop.state.account || "", shell.desktop.state.server || "", shell.route].join("/")
                        browsing: !!shell.desktop.state.historyBrowsing
                        delegate: ItemDelegate {
                            id: message
                            objectName: "messageRow"
                            required property var row
                            readonly property var person: shell.desktop.state.people ? shell.desktop.state.people[row.account] || ({}) : ({})
                            width: messages.width - messages.leftMargin - messages.rightMargin
                            visible: !messageSearch.text || row.body.toLowerCase().includes(messageSearch.text.toLowerCase())
                            height: visible ? implicitHeight : 0
                            padding: 8
                            hoverEnabled: true
                            HoverHandler { id: messageHover }
                            background: Rectangle {
                                color: messageHover.hovered ? Theme.navigation : "transparent"
                                radius: Theme.radius
                            }
                            contentItem: RowLayout {
                                spacing: 12
                                CordsButton {
                                    Layout.preferredWidth: 40
                                    Layout.preferredHeight: 40
                                    Layout.alignment: Qt.AlignTop
                                    padding: 0
                                    quiet: true
                                    text: message.row.author || "Verified sender"
                                    help: "Open account profile"
                                    enabled: !!message.row.account
                                    onClicked: shell.openProfile(message.row.account)
                                    contentItem: CordsAvatar {
                                        name: message.row.author
                                        account: message.row.account
                                        source: message.person.image || ""
                                        shape: message.person.shape || "circle"
                                        playing: messageHover.hovered && message.visible && message.y + message.height >= messages.contentY && message.y <= messages.contentY + messages.height
                                    }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2
                                    RowLayout {
                                        CordsButton {
                                            text: message.row.author || "Verified sender"
                                            quiet: true
                                            font.bold: true
                                            padding: 0
                                            implicitHeight: 22
                                            Layout.maximumWidth: Math.max(90, message.width * 0.45)
                                            enabled: !!message.row.account
                                            onClicked: shell.openProfile(message.row.account)
                                        }
                                        CordsLabel {
                                            text: new Date(message.row.timestamp * 1000).toLocaleString()
                                            font.pixelSize: Theme.captionSize
                                            color: Theme.muted
                                            Layout.fillWidth: true
                                            Layout.minimumWidth: 0
                                            wrapMode: Text.NoWrap
                                            elide: Text.ElideRight
                                        }
                                    }
                                    TextArea {
                                        text: message.row.body
                                        textFormat: TextEdit.PlainText
                                        readOnly: true
                                        selectByMouse: true
                                        wrapMode: TextEdit.Wrap
                                        color: Theme.text
                                        font.pixelSize: Theme.bodySize
                                        padding: 0
                                        Layout.fillWidth: true
                                        background: null
                                        Accessible.name: "Message from " + message.row.author
                                    }
                                }
                            }
                            TapHandler {
                                acceptedButtons: Qt.RightButton
                                onTapped: messageMenu.popup()
                            }
                            Menu {
                                id: messageMenu
                                MenuItem {
                                    text: "Copy message"
                                    onTriggered: shell.desktop.copy(message.row.body)
                                }
                                MenuItem {
                                    text: "Copy message ID"
                                    onTriggered: shell.desktop.copy(message.row.id)
                                }
                            }
                        }
                        ScrollBar.vertical: ScrollBar {
                            onPressedChanged: messages.followTail = !pressed && messages.atYEnd && !messages.browsing
                        }
                        CordsLabel {
                            anchors.centerIn: parent
                            width: Math.min(300, parent.width - 30)
                            horizontalAlignment: Text.AlignHCenter
                            visible: messages.count === 0
                            text: shell.route ? "No locally available messages. Synchronize or join this channel to receive new content." : "Select a server and channel to start a conversation."
                            color: Theme.secondary
                        }
                    }
                    CordsLabel {
                        visible: shell.channel.mode === "public"
                        text: "Public channel · this server can read messages"
                        color: Theme.warning
                        Layout.fillWidth: true
                        Layout.leftMargin: 16
                    }
                    CordsButton {
                        text: "Acknowledge public successor"
                        visible: !!shell.channel.acknowledgement
                        enabled: !shell.desktop.busy
                        onClicked: shell.confirm("Public successor", "This is a different channel identity. Its messages are readable by the server. Continue only if you accept this change.", function () {
                            shell.desktop.acknowledgePublic(shell.route);
                        })
                    }
                    CordsButton {
                        text: "Join encrypted conversation"
                        visible: !!shell.channel.id && !shell.channel.joined && shell.channel.mode === "encrypted" && !shell.channel.retired
                        enabled: !shell.desktop.busy
                        onClicked: shell.desktop.joinChannel(shell.route)
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.margins: 14
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.min(140, Math.max(48, composer.contentHeight + 20))
                            color: Theme.elevated
                            radius: Theme.radius
                            border.color: composer.activeFocus ? Theme.focus : Theme.border
                            ScrollView {
                                anchors.fill: parent
                                TextArea {
                                    id: composer
                                    objectName: "messageComposer"
                                    placeholderText: shell.channel.retired ? "Archived channel is read-only" : shell.route ? "Message #" + (shell.channel.name || "channel") : "Choose a channel"
                                    textFormat: TextEdit.PlainText
                                    color: Theme.text
                                    placeholderTextColor: Theme.muted
                                    wrapMode: TextEdit.Wrap
                                    selectByMouse: true
                                    inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhSensitiveData
                                    enabled: !!shell.route && !shell.channel.retired && !shell.channel.acknowledgement && !shell.desktop.state.revoked && !shell.desktop.state.burned
                                    font.pixelSize: Theme.bodySize
                                    background: null
                                    Accessible.name: "Message composer"
                                    Keys.onReturnPressed: event => {
                                        if (!(event.modifiers & Qt.ShiftModifier)) {
                                            send.clicked();
                                            event.accepted = true;
                                        } else
                                            event.accepted = false;
                                    }
                                }
                            }
                        }
                        CordsButton {
                            id: send
                            objectName: "sendMessage"
                            text: "Send"
                            primary: true
                            enabled: composer.enabled && !shell.desktop.busy && composer.text.trim().length > 0
                            onClicked: {
                                if (!enabled)
                                    return;
                                shell.submittedBody = composer.text;
                                shell.submittedRoute = shell.route;
                                shell.desktop.sendMessage(shell.route, composer.text);
                            }
                        }
                    }
                }
                MemberList {
                    visible: shell.showMembers && shell.wide
                    desktop: shell.desktop
                    Layout.preferredWidth: 240
                    Layout.fillHeight: true
                    onProfileRequested: account => shell.openProfile(account)
                }
                ConversationInfo {
                    visible: shell.showInfo && shell.wide
                    desktop: shell.desktop
                    channel: shell.channel
                    route: shell.route
                    Layout.preferredWidth: 240
                    Layout.fillHeight: true
                }
            }
        }
    }
    Drawer {
        id: infoDrawer
        edge: Qt.RightEdge
        width: Math.min(300, shell.width - 40)
        height: shell.height
        ConversationInfo {
            anchors.fill: parent
            desktop: shell.desktop
            channel: shell.channel
            route: shell.route
        }
    }
    Drawer {
        id: memberDrawer
        edge: Qt.RightEdge
        width: Math.min(300, shell.width - 40)
        height: shell.height
        MemberList {
            anchors.fill: parent
            desktop: shell.desktop
            onProfileRequested: account => shell.openProfile(account)
        }
    }
    CordsDialog {
        id: joinServer
        title: "Connect to a Cords server"
        ColumnLayout {
            width: parent.width
            spacing: 12
            CordsLabel {
                text: "Cords validates HTTPS and pins the server signing identity. Verify the origin before trusting a new server."
                Layout.fillWidth: true
            }
            CordsField {
                id: origin
                placeholderText: "https://community.example:4848"
                Layout.fillWidth: true
            }
            CordsButton {
                text: "Verify and trust"
                primary: true
                enabled: !shell.desktop.busy && origin.text.trim().length > 0
                onClicked: {
                    shell.desktop.trustServer(origin.text.trim());
                    joinServer.close();
                }
            }
            CordsButton {
                text: "Cancel"
                onClicked: joinServer.close()
            }
        }
    }
    CordsDialog {
        id: createChannel
        title: "Create channel"
        ColumnLayout {
            width: parent.width
            spacing: 12
            CordsField {
                id: channelName
                placeholderText: "Channel name"
                maximumLength: 100
                Layout.fillWidth: true
            }
            CordsCheck {
                id: publicChannel
                text: "Public: messages readable by the server"
            }
            CordsLabel {
                text: "Confidentiality is permanent for this channel identity."
                Layout.fillWidth: true
            }
            CordsButton {
                text: "Create"
                primary: true
                enabled: !shell.desktop.busy && channelName.text.trim().length > 0
                onClicked: {
                    shell.desktop.createChannel(channelName.text, publicChannel.checked);
                    createChannel.close();
                }
            }
            CordsButton {
                text: "Cancel"
                onClicked: createChannel.close()
            }
        }
    }
    ProfilePopup {
        id: profile
        desktop: shell.desktop
    }
    CordsDialog {
        id: activity
        title: "Pending activity"
        ColumnLayout {
            width: parent.width
            spacing: 10
            CordsLabel {
                text: "Choose a conversation to open. Notifications hide message content unless you enabled previews."
                Layout.fillWidth: true
            }
            ListView {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(300, Math.max(80, contentHeight))
                model: shell.desktop.notifications
                clip: true
                delegate: CordsButton {
                    required property var row
                    width: ListView.view.width
                    text: row.label
                    help: row.label
                    enabled: !shell.desktop.busy
                    onClicked: {
                        shell.desktop.activateAlert(row.id);
                        activity.close();
                    }
                }
                ScrollBar.vertical: ScrollBar {}
            }
            CordsLabel {
                visible: shell.desktop.notifications.count === 0
                text: "No pending activity."
            }
            CordsButton {
                text: "Close"
                onClicked: activity.close()
            }
        }
    }
    ConfirmDialog {
        id: confirmation
    }
}
