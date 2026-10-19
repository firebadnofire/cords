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
    property string submittedScope: ""
    // Sensitive presentation state stays in this unlocked shell's memory.
    // Destroying the shell on lock drops every draft; never write plaintext to Settings.
    property var drafts: ({})
    property string editingScope: ""
    property int matchingMessages: 0
    function updateMessageMatches() {
        const query = messageSearch.text.toLowerCase();
        let count = 0;
        for (let index = 0; index < desktop.messages.count; ++index) {
            if (desktop.messages.get(index).body.toLowerCase().includes(query))
                ++count;
        }
        matchingMessages = count;
    }
    readonly property string draftScope: JSON.stringify([desktop.state.account || "", desktop.state.server || "", route])
    readonly property string composerRestriction: desktop.state.burned ? "This identity has been burned. Sending is unavailable." : desktop.state.revoked ? "This device has been revoked. Sending is unavailable." : desktop.state.admission === "pending" ? "Waiting for administrator approval before you can send." : desktop.state.admission === "rejected" ? "Your membership request was denied. Submit a new request to join." : !route ? "Choose a channel to write a message." : channel.retired ? "This archived channel is read-only." : channel.acknowledgement ? "Acknowledge this channel's public successor before sending." : channel.mode === "encrypted" && !channel.joined ? "Join this encrypted conversation before sending." : ""
    function restoreDraft() {
        if (editingScope === draftScope)
            return;
        if (editingScope)
            drafts[editingScope] = composer.text;
        composer.text = drafts[draftScope] || "";
        messageSearch.clear();
        editingScope = draftScope;
    }
    Component.onCompleted: restoreDraft()
    Connections {
        target: shell.desktop.messages
        function onChanged() { shell.updateMessageMatches(); }
    }
    Shortcut {
        sequence: "Ctrl+F"
        enabled: !!shell.route && !shell.settingsOpen
        onActivated: {
            messageSearch.forceActiveFocus();
            messageSearch.selectAll();
        }
    }
    function continuesMessage(previous, current) {
        return !!current.account && current.account === previous.account
            && current.timestamp >= previous.timestamp && current.timestamp - previous.timestamp <= 300
            && new Date(previous.timestamp * 1000).toDateString() === new Date(current.timestamp * 1000).toDateString();
    }
    readonly property bool wide: width >= 1120
    readonly property string route: desktop.state.route || ""
    readonly property bool settingsOpen: showSettings || showAdmin
    function closeSettings() {
        if (showSettings)
            settingsPane.requestClose();
        else
            adminPane.requestClose();
    }
    Shortcut {
        sequence: "Escape"
        enabled: shell.settingsOpen && !profile.visible && !confirmation.visible && !serverClaim.visible && !settingsPane.modalOpen && !adminPane.modalOpen
        onActivated: shell.closeSettings()
    }
    Rectangle {
        anchors.fill: parent
        color: Theme.conversation
    }
    onWideChanged: {
        memberDrawer.close();
        infoDrawer.close();
    }
    function openProfile(account, anchor) {
        accountCard.openFor(account, anchor || null);
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
            shell.restoreDraft();
            if (shell.submittedBody && !shell.desktop.busy) {
                if (shell.desktop.state.notice === "Message sent") {
                    if (composer.text === shell.submittedBody && shell.editingScope === shell.submittedScope)
                        composer.clear();
                    if (shell.drafts[shell.submittedScope] === shell.submittedBody)
                        delete shell.drafts[shell.submittedScope];
                }
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
            visible: !shell.settingsOpen
            Layout.preferredWidth: Theme.serverIconSize + 24
            Layout.fillHeight: true
            color: Theme.rail
            ColumnLayout {
                anchors.fill: parent
                anchors.topMargin: 12
                anchors.bottomMargin: 12
                spacing: 8
                CordsButton {
                    text: "C"
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Theme.serverIconSize
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
                        height: Theme.serverIconSize
                        padding: 0
                        text: (row.name || row.origin.replace("https://", "")).slice(0, 2).toUpperCase()
                        Accessible.name: row.name || row.origin
                        primary: row.selected
                        contentItem: Item {
                            CordsAvatar {
                                objectName: "serverRailIcon"
                                anchors.centerIn: parent
                                width: Theme.serverIconSize
                                height: width
                                name: serverButton.text
                                account: shell.desktop.state.account || ""
                                source: serverButton.row.image || ""
                                shape: serverButton.row.shape || "circle"
                                playing: serverButton.visible && serverButton.y + serverButton.height >= serverList.contentY && serverButton.y <= serverList.contentY + serverList.height
                            }
                        }
                        background: Rectangle {
                            width: Theme.serverIconSize
                            height: width
                            x: (serverButton.width - width) / 2
                            y: (serverButton.height - height) / 2
                            radius: serverButton.row.selected || serverButton.hovered ? 13 : width / 2
                            color: serverButton.row.selected ? Theme.accent : serverButton.hovered ? Theme.hover : Theme.elevated
                            border.width: serverButton.visualFocus ? 2 : 0
                            border.color: Theme.focus
                            Behavior on radius {
                                NumberAnimation {
                                    duration: Theme.reducedMotion ? 0 : 120
                                }
                            }
                        }
                        Rectangle {
                            anchors.left: parent.left
                            anchors.leftMargin: 0
                            anchors.verticalCenter: parent.verticalCenter
                            width: 3
                            height: serverButton.row.selected ? 32 : 16
                            radius: 2
                            visible: serverButton.row.selected || serverButton.hovered
                            color: Theme.text
                        }
                        help: row.origin + (row.archived ? " (archived)" : row.error ? " · " + row.error : row.state === "pending" ? " · Waiting for administrator approval" : row.state === "rejected" ? " · Membership denied" : row.connected ? " · Connected" : " · Not connected")
                        Rectangle {
                            visible: !!serverButton.row.unread
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
                            if (row.selected && shell.desktop.state.ownership === "UNCLAIMED")
                                serverClaim.open();
                        }
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            onTapped: serverMenu.popup()
                        }
                        CordsMenu {
                            id: serverMenu
                            CordsMenuItem {
                                text: "Copy server origin"
                                onTriggered: shell.desktop.copy(serverButton.row.origin)
                            }
                            CordsMenuItem {
                                text: "Server settings"
                                onTriggered: {
                                    shell.desktop.selectServer(serverButton.row.id);
                                    shell.showAdmin = true;
                                    shell.showSettings = false;
                                }
                            }
                            CordsMenuItem {
                                text: shell.desktop.state.serverMuted ? "Unmute server notifications" : "Mute server notifications"
                                enabled: serverButton.row.selected && !shell.desktop.busy
                                onTriggered: shell.desktop.muteServer(!shell.desktop.state.serverMuted)
                            }
                            CordsMenuItem {
                                text: "Remove locally…"
                                onTriggered: shell.confirm("Remove server locally", "Remove " + serverButton.row.origin + " from this account's local state. This does not prove remote departure.", function () {
                                    shell.desktop.removeServer(serverButton.row.id, true);
                                })
                            }
                            CordsMenuItem {
                                text: "Verified departure…"
                                onTriggered: shell.confirm("Leave server", "Send a signed departure to " + serverButton.row.origin + " and remove local server state after verification.", function () {
                                    shell.desktop.removeServer(serverButton.row.id, false);
                                })
                            }
                            CordsMenuItem {
                                text: "Archive…"
                                onTriggered: shell.confirm("Archive server", "Leave " + serverButton.row.origin + " and keep a sealed local archive.", function () {
                                    shell.desktop.archiveServer(serverButton.row.id);
                                })
                            }
                        }
                    }
                    ScrollBar.vertical: ScrollBar {}
                }
                CordsIconButton {
                    iconName: "plus"
                    text: "Connect to a Cords server"
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Theme.serverIconSize
                    help: "Connect to a Cords server"
                    enabled: !shell.desktop.busy
                    onClicked: joinServer.open()
                }
                CordsIconButton {
                    iconName: "switch"
                    text: "Switch identity"
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Theme.serverIconSize
                    help: "Switch identity and lock this account (Ctrl+L)"
                    onClicked: shell.desktop.lockAccount(false)
                }
            }
        }
        Rectangle {
            visible: !shell.settingsOpen
            Layout.preferredWidth: shell.width < 850 ? 176 : 240
            Layout.fillHeight: true
            color: Theme.navigation
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 10
                CordsButton {
                    id: serverHeader
                    text: (shell.desktop.state.serverProfile || {}).name || (shell.desktop.state.origin ? shell.desktop.state.origin.replace("https://", "") : "Choose a server")
                    quiet: true
                    font.bold: true
                    implicitHeight: 44
                    Layout.fillWidth: true
                    contentItem: RowLayout {
                        CordsLabel {
                            text: serverHeader.text
                            font.bold: true
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            wrapMode: Text.NoWrap
                        }
                        CordsIcon { name: "chevron" }
                    }
                    help: shell.desktop.state.origin || "Connect using + in the server rail"
                    onClicked: serverHeaderMenu.popup()
                    CordsMenu {
                        id: serverHeaderMenu
                        CordsMenuItem {
                            text: "Synchronize server"
                            enabled: !shell.desktop.busy && !!shell.desktop.state.server
                            onTriggered: shell.desktop.synchronize()
                        }
                        CordsMenuItem {
                            text: "Server settings"
                            enabled: !!shell.desktop.state.server
                            onTriggered: {
                                shell.showAdmin = true;
                                shell.showSettings = false;
                            }
                        }
                        CordsMenuItem {
                            text: shell.desktop.state.ownership === "UNCLAIMED" ? "Claim server…" : "Authenticate / request admission"
                            enabled: !shell.desktop.busy && !!shell.desktop.state.server
                            onTriggered: {
                                if (shell.desktop.state.ownership === "UNCLAIMED")
                                    serverClaim.open();
                                else
                                    shell.desktop.authenticate();
                            }
                        }
                        CordsMenuItem {
                            text: shell.desktop.state.serverMuted ? "Unmute server" : "Mute server"
                            enabled: !shell.desktop.busy && !!shell.desktop.state.server
                            onTriggered: shell.desktop.muteServer(!shell.desktop.state.serverMuted)
                        }
                    }
                }
                CordsLabel {
                    text: shell.desktop.state.ownership === "UNCLAIMED" ? "Unclaimed server · owner setup required" : ({
                            pending: "Waiting for administrator approval",
                            rejected: "Membership request denied",
                            active: "Membership approved"
                        })[shell.desktop.state.admission] || "Not authenticated"
                    visible: shell.desktop.state.admission !== "active"
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                }
                CordsButton {
                    text: shell.desktop.state.admission === "pending" ? "Check approval status" : shell.desktop.state.admission === "rejected" ? "Submit a new request" : "Authenticate / request admission"
                    visible: !!shell.desktop.state.server && !shell.desktop.state.connected
                    Layout.fillWidth: true
                    enabled: !shell.desktop.busy
                    onClicked: {
                        if (shell.desktop.state.ownership === "UNCLAIMED")
                            serverClaim.open();
                        else
                            shell.desktop.authenticate();
                    }
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
                        id: channelButton
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
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            onTapped: channelMenu.popup()
                        }
                        CordsMenu {
                            id: channelMenu
                            CordsMenuItem {
                                text: "Open channel"
                                enabled: !shell.desktop.busy
                                onTriggered: shell.desktop.selectChannel(channelButton.row.id)
                            }
                            CordsMenuItem {
                                text: shell.desktop.state.routeMuted ? "Unmute conversation" : "Mute conversation"
                                enabled: !shell.desktop.busy && channelButton.row.id === shell.route
                                onTriggered: shell.desktop.muteRoute(channelButton.row.id, !shell.desktop.state.routeMuted)
                            }
                        }
                    }
                    ScrollBar.vertical: ScrollBar {}
                }
                CordsButton {
                    text: shell.desktop.state.preferences ? shell.desktop.state.preferences.name : "Your account"
                    selected: true
                    quiet: true
                    implicitHeight: 52
                    contentItem: RowLayout {
                        spacing: 10
                        CordsAvatar {
                            name: shell.desktop.state.preferences ? shell.desktop.state.preferences.name || "Your account" : "Your account"
                            account: shell.desktop.state.account || ""
                            Layout.preferredWidth: 30
                            Layout.preferredHeight: 30
                        }
                        CordsLabel {
                            text: shell.desktop.state.preferences ? shell.desktop.state.preferences.name || "Your account" : "Your account"
                            font.bold: true
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            wrapMode: Text.NoWrap
                        }
                        CordsIcon {
                            name: "settings"
                        }
                    }
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
                Layout.preferredHeight: Theme.headerHeight
                color: Theme.conversation
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
                        objectName: "messageSearch"
                        placeholderText: "Search loaded messages"
                        help: "Filters the message bodies currently loaded in this conversation (Ctrl+F)"
                        onTextChanged: shell.updateMessageMatches()
                        Keys.onEscapePressed: clear()
                        Layout.preferredWidth: shell.width < 850 ? 120 : 210
                        visible: shell.route.length > 0 && !shell.showSettings && !shell.showAdmin
                    }
                    CordsButton {
                        text: "Activity"
                        visible: !shell.showSettings && !shell.showAdmin && shell.desktop.notifications.count > 0
                        help: "Open pending message notifications"
                        onClicked: activity.open()
                    }
                    CordsIconButton {
                        iconName: "users"
                        text: "Members"
                        quiet: true
                        selected: shell.showMembers
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
                    CordsIconButton {
                        iconName: "info"
                        text: "Info"
                        quiet: true
                        selected: shell.showInfo
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
                        text: "×  Close"
                        quiet: true
                        help: "Return to chat (Escape)"
                        visible: shell.showSettings || shell.showAdmin
                        onClicked: shell.closeSettings()
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
                text: shell.desktop.state.error === "CORDS_SERVER_UNCLAIMED" ? "This server needs an owner. Choose Authenticate to claim it, or ask its operator to finish setup." : shell.desktop.state.error || ""
                color: Theme.danger
                visible: text.length > 0
                Layout.fillWidth: true
                padding: 10
                Accessible.role: Accessible.AlertMessage
            }
            CordsLabel {
                text: shell.desktop.state.notice || ""
                color: Theme.secondary
                visible: text.length > 0 && !["Account unlocked", "Synchronized", "Message sent"].includes(text)
                Layout.fillWidth: true
                padding: 8
            }
            SettingsPane {
                id: settingsPane
                onCloseRequested: {
                    shell.showSettings = false;
                    composer.forceActiveFocus();
                }
                visible: shell.showSettings
                desktop: shell.desktop
                Layout.fillWidth: true
                Layout.fillHeight: true
                onProfileRequested: {
                    profile.account = shell.desktop.state.account;
                    profile.open();
                }
            }
            AdminPane {
                id: adminPane
                onCloseRequested: {
                    shell.showAdmin = false;
                    composer.forceActiveFocus();
                }
                visible: shell.showAdmin
                desktop: shell.desktop
                route: shell.route
                Layout.fillWidth: true
                Layout.fillHeight: true
                onProfileRequested: account => shell.openProfile(account)
                onClaimRequested: serverClaim.open()
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
                        visible: !!shell.route && (!!shell.desktop.state.historyOlder || !!shell.desktop.state.historyBrowsing)
                        spacing: 8
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
                        spacing: 0
                        leftMargin: 14
                        rightMargin: 14
                        historyScope: [shell.desktop.state.account || "", shell.desktop.state.server || "", shell.route].join("/")
                        browsing: !!shell.desktop.state.historyBrowsing
                        delegate: ItemDelegate {
                            id: message
                            objectName: "messageRow"
                            required property var row
                            required property int index
                            readonly property var previous: index > 0 ? shell.desktop.messages.get(index - 1) : ({})
                            readonly property bool grouped: !messageSearch.text && shell.continuesMessage(previous, row)
                            readonly property bool dayStart: index === 0 || new Date(previous.timestamp * 1000).toDateString() !== new Date(row.timestamp * 1000).toDateString()
                            readonly property var person: shell.desktop.state.people ? shell.desktop.state.people[row.account] || ({}) : ({})
                            width: messages.width - messages.leftMargin - messages.rightMargin
                            visible: !messageSearch.text || row.body.toLowerCase().includes(messageSearch.text.toLowerCase())
                            height: visible ? implicitHeight : 0
                            padding: 8
                            topPadding: grouped ? 2 : 12
                            bottomPadding: 2
                            hoverEnabled: true
                            Accessible.name: "Message from " + row.author + ", " + new Date(row.timestamp * 1000).toLocaleString()
                            Accessible.description: row.body
                            Keys.onMenuPressed: messageMenu.popup()
                            HoverHandler {
                                id: messageHover
                            }
                            background: Rectangle {
                                color: messageHover.hovered ? Theme.navigation : "transparent"
                                radius: Theme.radius
                            }
                            contentItem: ColumnLayout {
                                spacing: 8
                                RowLayout {
                                    visible: message.dayStart && !messageSearch.text
                                    Layout.fillWidth: true
                                    spacing: 12
                                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.divider }
                                    CordsLabel { text: new Date(message.row.timestamp * 1000).toLocaleDateString(Qt.locale(), Locale.LongFormat); color: Theme.muted; font.pixelSize: Theme.captionSize }
                                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.divider }
                                }
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 12
                                    Item {
                                        Layout.preferredWidth: 40
                                        Layout.preferredHeight: message.grouped ? Theme.bodySize + 6 : 40
                                        Layout.alignment: Qt.AlignTop
                                        CordsButton {
                                            id: messageAvatarButton
                                            anchors.fill: parent
                                            visible: !message.grouped
                                            padding: 0
                                            quiet: true
                                            text: message.row.author || "Verified sender"
                                            help: "Open account profile"
                                            enabled: !!message.row.account
                                            onClicked: shell.openProfile(message.row.account, messageAvatarButton)
                                            contentItem: CordsAvatar {
                                                name: message.row.author
                                                account: message.row.account
                                                source: message.person.image || ""
                                                shape: message.person.shape || "circle"
                                                playing: messageHover.hovered && message.visible && !message.grouped && message.y + message.height >= messages.contentY && message.y <= messages.contentY + messages.height
                                            }
                                        }
                                        CordsLabel {
                                            anchors.centerIn: parent
                                            visible: message.grouped && (messageHover.hovered || message.visualFocus)
                                            text: new Date(message.row.timestamp * 1000).toLocaleTimeString(Qt.locale(), Locale.ShortFormat)
                                            color: Theme.muted
                                            font.pixelSize: Theme.captionSize
                                        }
                                    }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 2
                                        RowLayout {
                                            visible: !message.grouped
                                            CordsButton {
                                                id: messageAuthorButton
                                                text: message.row.author || "Verified sender"
                                                quiet: true
                                                font.bold: true
                                                padding: 0
                                                implicitHeight: Theme.bodySize + 8
                                                Layout.maximumWidth: Math.max(90, message.width * 0.45)
                                                enabled: !!message.row.account
                                                onClicked: shell.openProfile(message.row.account, messageAuthorButton)
                                            }
                                            CordsLabel {
                                                text: new Date(message.row.timestamp * 1000).toLocaleTimeString(Qt.locale(), Locale.ShortFormat)
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
                                            Accessible.name: "Message from " + message.row.author + ", " + new Date(message.row.timestamp * 1000).toLocaleString()
                                            Accessible.description: message.row.body
                                        }
                                    }
                                }
                            }
                            TapHandler {
                                acceptedButtons: Qt.RightButton
                                onTapped: messageMenu.popup()
                            }
                            CordsIconButton {
                                iconName: "more"
                                text: "Message actions"
                                visible: messageHover.hovered || message.visualFocus || messageMenu.opened
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.topMargin: message.dayStart ? 28 : 0
                                implicitWidth: 32
                                implicitHeight: 32
                                onClicked: messageMenu.popup()
                            }
                            CordsMenu {
                                id: messageMenu
                                objectName: "messageContextMenu"
                                CordsMenuItem {
                                    text: "Copy message"
                                    onTriggered: shell.desktop.copy(message.row.body)
                                }
                                CordsMenuItem {
                                    text: "Copy message ID"
                                    onTriggered: shell.desktop.copy(message.row.id)
                                }
                            }
                        }
                        ScrollBar.vertical: ScrollBar {
                            onPressedChanged: messages.followTail = !pressed && messages.atYEnd && !messages.browsing
                        }
                        CordsEmptyState {
                            anchors.centerIn: parent
                            objectName: "conversationEmptyState"
                            width: Math.min(440, parent.width - 48)
                            visible: messages.count === 0 || !!messageSearch.text && shell.matchingMessages === 0
                            title: messageSearch.text ? "No loaded messages match" : shell.desktop.state.admission === "pending" ? "Waiting for administrator approval" : shell.desktop.state.admission === "rejected" ? "Your request to join was denied" : shell.route ? "Messages haven't been loaded on this device" : "Choose a conversation"
                            explanation: messageSearch.text ? "Search covers only the message bodies currently loaded in this conversation. Load older history to expand its scope." : shell.desktop.state.admission === "pending" ? "Your request to join " + shell.desktop.state.origin + " has been submitted. An administrator must approve it before you can access this server." : shell.desktop.state.admission === "rejected" ? "An administrator declined your request to join " + shell.desktop.state.origin + ". You can submit a new request." : shell.route ? "This device has no locally available messages for this channel. Load messages when connected; an empty cache does not mean the conversation is empty." : "Select a server and channel from the sidebar."
                            actionText: messageSearch.text ? "Clear search" : ["pending", "rejected"].includes(shell.desktop.state.admission) ? (shell.desktop.state.admission === "pending" ? "Check approval status" : "Submit a new request") : shell.route && shell.desktop.state.connected ? "Load messages" : ""
                            busy: !messageSearch.text && shell.desktop.busy
                            onActionRequested: {
                                if (messageSearch.text)
                                    messageSearch.clear();
                                else if (["pending", "rejected"].includes(shell.desktop.state.admission))
                                    shell.desktop.authenticate();
                                else
                                    shell.desktop.synchronize();
                            }
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
                    Rectangle {
                        objectName: "composerSurface"
                        Layout.fillWidth: true
                        Layout.leftMargin: 16
                        Layout.rightMargin: 16
                        Layout.preferredHeight: Math.min(160, Math.max(52, composer.contentHeight + 24))
                        color: Theme.input
                        radius: Theme.popoverRadius
                        border.width: composer.activeFocus ? 2 : 1
                        border.color: composer.activeFocus ? Theme.focus : Theme.divider
                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 6
                            spacing: 4
                            ScrollView {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                contentWidth: availableWidth
                                clip: true
                                TextArea {
                                    id: composer
                                    objectName: "messageComposer"
                                    placeholderText: shell.route ? "Message #" + (shell.channel.name || "channel") : "Choose a channel"
                                    textFormat: TextEdit.PlainText
                                    color: Theme.text
                                    placeholderTextColor: Theme.muted
                                    wrapMode: TextEdit.Wrap
                                    selectByMouse: true
                                    inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhSensitiveData
                                    enabled: !shell.composerRestriction
                                    font.pixelSize: Theme.bodySize
                                    padding: 10
                                    background: null
                                    Accessible.name: "Message composer"
                                    Accessible.description: shell.composerRestriction || "Enter to send. Shift+Enter for a new line."
                                    Keys.onReturnPressed: event => {
                                        if (!(event.modifiers & Qt.ShiftModifier)) {
                                            send.clicked();
                                            event.accepted = true;
                                        } else
                                            event.accepted = false;
                                    }
                                }
                            }
                            CordsIconButton {
                                id: send
                                objectName: "sendMessage"
                                iconName: "send"
                                text: "Send message"
                                quiet: true
                                Layout.alignment: Qt.AlignBottom
                                enabled: composer.enabled && !!shell.desktop.state.connected && !shell.desktop.busy && composer.text.trim().length > 0
                                help: shell.composerRestriction || (!shell.desktop.state.connected ? "Reconnect to send this draft" : "Send message (Enter)")
                                onClicked: {
                                    if (!enabled)
                                        return;
                                    shell.submittedBody = composer.text;
                                    shell.submittedScope = shell.editingScope;
                                    shell.desktop.sendMessage(shell.route, composer.text);
                                }
                            }
                        }
                    }
                    CordsLabel {
                        objectName: "composerFeedback"
                        text: shell.composerRestriction || (shell.route && !shell.desktop.state.connected ? "Offline. Your draft stays in this unlocked session; reconnect to send." : "Enter to send · Shift+Enter for a new line")
                        color: Theme.muted
                        font.pixelSize: Theme.captionSize
                        Layout.fillWidth: true
                        Layout.leftMargin: 16
                        Layout.rightMargin: 16
                        Layout.bottomMargin: 12
                    }
                }
                MemberList {
                    id: memberRail
                    visible: shell.showMembers && shell.wide
                    desktop: shell.desktop
                    Layout.preferredWidth: 240
                    Layout.fillHeight: true
                    onProfileRequested: (account, anchor) => shell.openProfile(account, anchor)
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
            id: drawerMembers
            anchors.fill: parent
            desktop: shell.desktop
            onProfileRequested: (account, anchor) => shell.openProfile(account, anchor)
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
    ServerClaim {
        id: serverClaim
        server: shell.desktop.state.server || ""
        ownership: shell.desktop.state.ownership || ""
        origin: shell.desktop.state.origin || ""
        operationBusy: shell.desktop.busy
        errorText: shell.desktop.state.error || ""
        onClaimRequested: code => shell.desktop.claimOwnership(code)
    }
    AccountCard {
        id: accountCard
        desktop: shell.desktop
        onRestoreMemberFocus: account => {
            if (shell.wide && shell.showMembers)
                memberRail.restoreAccountFocus(account);
            else if (memberDrawer.opened)
                drawerMembers.restoreAccountFocus(account);
        }
        onFullProfileRequested: account => {
            profile.account = account;
            profile.open();
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
