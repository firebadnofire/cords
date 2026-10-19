pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: card
    required property Controller desktop
    property string account: ""
    property Item returnFocus: null
    property bool returnToMember: false
    signal restoreMemberFocus(string account)
    property point anchorPoint: Qt.point(0, 0)
    property bool details: false
    property bool playbackPaused: false
    property bool manualPlayback: false
    readonly property var profile: desktop.state.profile || ({})
    readonly property bool loaded: profile.account === account
    readonly property string scope: JSON.stringify([desktop.state.account || "", desktop.state.server || ""])
    onScopeChanged: close()
    readonly property var visibleRoles: {
        const currentScope = scope;
        const result = [];
        for (let index = 0; index < desktop.members.count; ++index) {
            const member = desktop.members.get(index);
            if (member.id !== account)
                continue;
            for (const id of member.roles || []) {
                for (let roleIndex = 0; roleIndex < desktop.roles.count; ++roleIndex) {
                    const role = desktop.roles.get(roleIndex);
                    if (role.id === id)
                        result.push(role);
                }
            }
        }
        return result;
    }
    signal fullProfileRequested(string account)
    objectName: "accountCard"
    parent: Overlay.overlay
    width: Math.min(336, parent ? parent.width - 24 : 336)
    height: Math.min(implicitHeight, parent ? parent.height - 24 : implicitHeight)
    x: Math.max(12, Math.min(anchorPoint.x, parent ? parent.width - width - 12 : anchorPoint.x))
    y: Math.max(12, Math.min(anchorPoint.y, parent ? parent.height - height - 12 : anchorPoint.y))
    padding: 16
    focus: true
    modal: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    function openFor(identity, anchor) {
        account = identity;
        returnFocus = anchor;
        returnToMember = !!anchor && anchor.objectName === "memberRow";
        if (anchor && parent) {
            const location = anchor.mapToItem(parent, 0, 0);
            anchorPoint = Qt.point(location.x + anchor.width + width + 20 <= parent.width ? location.x + anchor.width + 8 : location.x - width - 8, location.y);
        } else {
            anchorPoint = Qt.point(parent ? (parent.width - width) / 2 : 12, 72);
        }
        open();
    }
    onOpened: {
        details = false;
        playbackPaused = false;
        manualPlayback = false;
        if (account && !desktop.busy)
            desktop.loadProfile(account);
    }
    onClosed: {
        playbackPaused = true;
        manualPlayback = false;
        // The authoritative refresh can replace the delegate that opened us.
        // Restore after Qt finishes closing the popup, using identity if needed.
        const closedAccount = account;
        const closedScope = scope;
        Qt.callLater(() => {
            if (card.opened || card.scope !== closedScope)
                return;
            if (card.returnToMember)
                card.restoreMemberFocus(closedAccount);
            else if (card.returnFocus && card.returnFocus.visible && card.returnFocus.enabled)
                card.returnFocus.forceActiveFocus();
        });
    }
    Connections {
        target: Theme
        function onReducedMotionChanged() {
            card.manualPlayback = false;
        }
        function onDisableAnimationsChanged() {
            card.manualPlayback = false;
        }
    }
    background: Rectangle {
        color: Theme.elevated
        radius: Theme.popoverRadius
        border.color: Theme.divider
    }
    contentHeight: body.implicitHeight
    contentItem: ScrollView {
        id: scroll
        contentWidth: availableWidth
        clip: true
        ColumnLayout {
            id: body
            width: scroll.availableWidth
            spacing: 12
            RowLayout {
                Layout.fillWidth: true
                CordsAvatar {
                    id: avatar
                    objectName: "accountCardAvatar"
                    account: card.account
                    name: card.loaded ? card.profile.name || "Verified account" : "Verified account"
                    source: card.loaded ? card.profile.image || "" : ""
                    shape: card.loaded ? card.profile.shape || "circle" : "circle"
                    Layout.preferredWidth: 64
                    Layout.preferredHeight: 64
                    playing: card.opened && !card.playbackPaused
                    motionOverride: card.manualPlayback
                }
                Item {
                    Layout.fillWidth: true
                }
                CordsIconButton {
                    objectName: "accountCardClose"
                    iconName: "close"
                    text: "Close account card"
                    onClicked: card.close()
                }
            }
            CordsLabel {
                text: card.loaded && card.profile.name ? card.profile.name : "Verified account"
                font.pixelSize: Theme.headingSize
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6
                visible: card.visibleRoles.length > 0
                Repeater {
                    model: card.visibleRoles
                    CordsRoleChip {
                        required property var modelData
                        roleName: modelData.name
                        roleColor: modelData.color
                    }
                }
            }
            CordsLabel {
                visible: !card.loaded
                text: card.desktop.busy ? "Loading verified profile…" : "Profile could not be retrieved. Retry when connected."
                color: Theme.secondary
                Layout.fillWidth: true
            }
            CordsLabel {
                text: card.desktop.state.error || (card.loaded ? card.profile.pending || "" : "")
                visible: !!text
                color: Theme.warning
                Layout.fillWidth: true
            }
            CordsButton {
                text: "Retry profile retrieval"
                visible: !card.loaded && !card.desktop.busy
                enabled: !!card.account
                onClicked: card.desktop.loadProfile(card.account)
            }
            CordsButton {
                text: card.playbackPaused || (Theme.reducedMotion && !card.manualPlayback) || (Theme.disableAnimations && !card.manualPlayback) ? "Play avatar" : "Pause avatar"
                visible: card.loaded && avatar.animated
                enabled: !avatar.imageError
                onClicked: {
                    const running = !card.playbackPaused && (card.manualPlayback || (!Theme.reducedMotion && !Theme.disableAnimations));
                    card.playbackPaused = running;
                    card.manualPlayback = !running;
                }
            }
            CordsLabel {
                text: avatar.imageError
                visible: !!text
                color: Theme.warning
                Layout.fillWidth: true
            }
            CordsButton {
                objectName: "accountCardDetails"
                text: card.details ? "Hide identity details" : "Identity details"
                quiet: true
                onClicked: card.details = !card.details
            }
            CordsField {
                visible: card.details
                text: card.account
                readOnly: true
                selectByMouse: true
                Layout.fillWidth: true
                Accessible.name: "Account fingerprint"
            }
            CordsButton {
                visible: card.details
                text: "Copy account fingerprint"
                onClicked: card.desktop.copy(card.account)
            }
            CordsButton {
                text: card.account === card.desktop.state.account ? "Edit account profile" : "Full verified profile"
                Layout.fillWidth: true
                onClicked: {
                    card.close();
                    card.fullProfileRequested(card.account);
                }
            }
        }
    }
}
