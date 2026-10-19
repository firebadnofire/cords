pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: form
    required property Controller desktop
    readonly property var profile: desktop.state.serverProfile || ({})
    readonly property string errorMessage: (desktop.state.error || "").includes("CORDS_SERVER_PROFILE_CONFLICT") ? "This server profile changed. Cancel the crop or discard your draft, then refresh and review the latest profile before retrying." : desktop.state.error || ""
    property string baselineName: ""
    property string baselineDescription: ""
    property string baselineBackground: "#314452"
    readonly property bool modalOpen: colorPicker.visible || iconFile.visible || iconEditor.visible || removeIcon.visible
    property var reviewedRevision: 0
    property string scope: ""
    property bool saving: false
    property string payload: ""
    property string requestId: ""
    readonly property bool dirty: name.text !== baselineName || description.text !== baselineDescription || backgroundColor.text !== baselineBackground
    readonly property bool editable: !!profile.loaded && !!profile.canEdit && !desktop.busy
    function reset() {
        baselineName = profile.name || "";
        baselineDescription = profile.description || "";
        baselineBackground = profile.background || "#314452";
        name.text = baselineName;
        description.text = baselineDescription;
        backgroundColor.text = baselineBackground;
        reviewedRevision = profile.revision || 0;
        scope = JSON.stringify([desktop.state.account || "", desktop.state.server || ""]);
        payload = "";
    }
    Component.onCompleted: reset()
    Connections {
        target: form.desktop
        function onStateChanged() {
            if (form.scope !== JSON.stringify([form.desktop.state.account || "", form.desktop.state.server || ""])) {
                form.saving = false;
                colorPicker.close();
                iconFile.close();
                removeIcon.close();
                removeIcon.operation = function () {};
                form.reset();
            } else if (form.saving && !form.desktop.busy) {
                form.saving = false;
                if (!form.desktop.state.error && form.desktop.state.notice === "Server profile saved")
                    form.reset();
            } else if (!form.dirty && !form.saving)
                form.reset();
        }
    }
    spacing: 14
    CordsLabel {
        text: "Server profile"
        font.pixelSize: Theme.headingSize
    }
    CordsLabel {
        text: form.desktop.state.origin || "Choose a server"
        Layout.fillWidth: true
        color: Theme.secondary
    }
    CordsLabel {
        text: !form.profile.supported ? "Profile editing is unavailable. Refresh to check server support." : form.desktop.state.admission !== "active" ? "Approved membership is required to view this server's profile." : !form.profile.loaded ? "Refresh to load the current server profile." : !form.profile.canEdit ? "Only authorized server staff can change this profile." : "Changes appear in discovery and members' server lists after saving."
        Layout.fillWidth: true
        color: Theme.secondary
    }
    CordsButton {
        text: form.desktop.busy ? "Working…" : "Refresh profile"
        enabled: !form.desktop.busy && !form.dirty && form.desktop.state.admission === "active"
        onClicked: form.desktop.loadServerProfile()
    }
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: preview.implicitHeight + 32
        color: Theme.elevated
        radius: Theme.radius
        ColumnLayout {
            id: preview
            anchors.fill: parent
            anchors.margins: 16
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 72
                radius: Theme.radius
                color: /^#[0-9a-fA-F]{6}$/.test(backgroundColor.text) ? backgroundColor.text : "#314452"
            }
            CordsAvatar {
                objectName: "serverProfileIcon"
                Layout.preferredWidth: 64
                Layout.preferredHeight: 64
                name: name.text
                account: form.desktop.state.account || ""
                source: form.profile.image || ""
                shape: form.profile.shape || "circle"
                playing: form.visible
            }
            CordsLabel {
                text: name.text || "Server preview"
                font.pixelSize: Theme.headingSize
                Layout.fillWidth: true
            }
            CordsLabel {
                text: description.text || "Add a description for your community."
                color: Theme.secondary
                Layout.fillWidth: true
            }
        }
    }
    CordsLabel {
        text: "Server icon"
        font.bold: true
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        CordsButton {
            text: "Change icon…"
            enabled: form.editable && !form.dirty
            onClicked: iconFile.open()
        }
        CordsButton {
            text: "Remove icon…"
            destructive: true
            enabled: form.editable && !form.dirty && !!form.profile.hasIcon
            onClicked: {
                const revision = form.profile.revision;
                const scope = form.scope;
                const requestId = form.desktop.newId();
                removeIcon.operation = function () {
                    if (form.scope === scope)
                        form.desktop.removeServerIcon(revision, requestId);
                };
                removeIcon.open();
            }
        }
    }
    CordsLabel {
        visible: form.dirty
        text: "Save or discard these edits before changing the icon."
        color: Theme.secondary
        Layout.fillWidth: true
    }
    FileDialog {
        id: iconFile
        title: "Choose a server icon"
        nameFilters: ["Pictures (*.jxl *.png *.jpg *.jpeg *.gif *.webp *.apng)"]
        onAccepted: form.desktop.prepareServerIcon(selectedFile)
    }
    AvatarEditor {
        id: iconEditor
        objectName: "serverIconEditor"
        parent: Overlay.overlay
        title: "Adjust server icon"
        draft: form.desktop.state.serverIconDraft || ({})
        operationBusy: form.desktop.busy
        errorText: form.errorMessage
        onChooseRequested: iconFile.open()
        onSaveRequested: (id, x, y, zoom, shape) => form.desktop.saveServerIcon(id, x, y, zoom, shape)
        onCancelRequested: if (!form.desktop.busy)
            form.desktop.cancelServerIcon()
    }
    ConfirmDialog {
        id: removeIcon
        parent: Overlay.overlay
        title: "Remove server icon?"
        explanation: "The server will stop supplying this icon. Members may retain previously downloaded copies."
        confirmText: "Remove icon"
    }
    CordsLabel {
        text: "Profile background"
        font.bold: true
    }
    RowLayout {
        Layout.fillWidth: true
        CordsField {
            id: backgroundColor
            objectName: "serverProfileBackground"
            Layout.fillWidth: true
            maximumLength: 7
            readOnly: !form.editable
            Accessible.name: "Profile background color in hexadecimal"
        }
        CordsButton {
            text: "Choose color…"
            enabled: form.editable
            onClicked: {
                colorPicker.selectedColor = /^#[0-9a-fA-F]{6}$/.test(backgroundColor.text) ? backgroundColor.text : "#314452";
                colorPicker.open();
            }
        }
    }
    ColorDialog {
        id: colorPicker
        title: "Server profile background"
        onAccepted: backgroundColor.text = selectedColor.toString()
    }
    CordsLabel {
        text: "Use #RRGGBB. The background changes the profile banner; text keeps the application theme for readability."
        Layout.fillWidth: true
        color: Theme.secondary
    }
    CordsLabel {
        text: "Name"
        font.bold: true
    }
    CordsField {
        id: name
        objectName: "serverProfileName"
        Layout.fillWidth: true
        maximumLength: 100
        readOnly: !form.editable
        Accessible.name: "Server name"
    }
    CordsLabel {
        text: "Description"
        font.bold: true
    }
    TextArea {
        id: description
        objectName: "serverProfileDescription"
        Layout.fillWidth: true
        Layout.preferredHeight: 120
        wrapMode: TextEdit.Wrap
        readOnly: !form.editable
        color: Theme.text
        selectionColor: Theme.accent
        font.pixelSize: Theme.bodySize
        selectByMouse: true
        Accessible.name: "Server description"
        background: Rectangle {
            color: Theme.rail
            radius: 6
            border.color: Theme.border
        }
    }
    CordsLabel {
        text: Array.from(description.text).length + " / 2000 characters"
        color: Theme.secondary
    }
    CordsLabel {
        visible: form.dirty
        text: form.reviewedRevision !== form.profile.revision ? "This profile changed on the server. Discard your draft and refresh before saving." : "Unsaved changes. Save or discard before leaving this page."
        color: Theme.warning
        Layout.fillWidth: true
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        CordsButton {
            objectName: "saveServerProfile"
            text: form.saving ? "Saving…" : "Save changes"
            primary: true
            enabled: form.editable && form.dirty && name.text.trim().length > 0 && /^#[0-9a-fA-F]{6}$/.test(backgroundColor.text) && Array.from(description.text).length <= 2000 && form.reviewedRevision === form.profile.revision
            onClicked: {
                const next = JSON.stringify([name.text, description.text, backgroundColor.text, form.reviewedRevision]);
                if (form.payload !== next) {
                    form.payload = next;
                    form.requestId = form.desktop.newId();
                }
                form.saving = true;
                form.desktop.saveServerProfile(name.text, description.text, backgroundColor.text, form.reviewedRevision, form.requestId);
            }
        }
        CordsButton {
            text: "Discard changes"
            enabled: !form.desktop.busy && form.dirty
            onClicked: form.reset()
        }
    }
    CordsLabel {
        text: form.errorMessage
        visible: !!text
        color: Theme.danger
        Layout.fillWidth: true
    }
    CordsLabel {
        text: "Server fingerprint"
        font.bold: true
    }
    CordsField {
        text: form.desktop.state.server || ""
        readOnly: true
        Layout.fillWidth: true
        Accessible.name: "Server fingerprint"
    }
    CordsButton {
        text: "Copy fingerprint"
        onClicked: form.desktop.copy(form.desktop.state.server)
    }
}
