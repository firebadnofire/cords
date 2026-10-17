import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CordsDialog {
    id: editor
    property var draft: ({})
    property bool operationBusy: false
    property string errorText: ""
    property real centerX: 0.5
    property real centerY: 0.5
    property string shape: "circle"
    readonly property real zoom: zoomControl.value
    readonly property string draftId: draft.id || ""
    readonly property real side: Math.min(draft.width || 1, draft.height || 1) / zoom
    signal saveRequested(string id, real x, real y, real zoom, string shape)
    signal chooseRequested
    signal cancelRequested
    objectName: "avatarEditor"
    title: "Adjust profile picture"
    width: Math.min(520, parent ? parent.width - 32 : 520)
    contentHeight: Math.min(editorContent.implicitHeight, parent ? parent.height - 150 : 520)
    closePolicy: editor.operationBusy ? Popup.NoAutoClose : Popup.CloseOnEscape
    onClosed: if (draftId) cancelRequested()
    function resetCrop() {
        centerX = 0.5;
        centerY = 0.5;
        zoomControl.value = 1;
    }
    function constrain() {
        const halfX = side / (2 * (draft.width || 1));
        const halfY = side / (2 * (draft.height || 1));
        centerX = Math.max(halfX, Math.min(1 - halfX, centerX));
        centerY = Math.max(halfY, Math.min(1 - halfY, centerY));
    }
    onDraftIdChanged: {
        resetCrop();
        if (draft.id)
            open();
        else
            close();
    }
    ScrollView {
        id: cropScroll
        anchors.fill: parent
        contentWidth: availableWidth
        clip: true
        ColumnLayout {
            id: editorContent
            width: cropScroll.availableWidth
            spacing: 12
            CordsLabel {
                text: "Drag to position your picture. Use the slider to zoom."
                color: Theme.secondary
                Layout.fillWidth: true
            }
            CordsAvatar {
                id: cropPreview
                objectName: "avatarCropPreview"
                Layout.preferredWidth: Math.min(280, cropScroll.availableWidth)
                Layout.preferredHeight: Layout.preferredWidth
                Layout.alignment: Qt.AlignHCenter
                source: editor.draft.image || ""
                shape: editor.shape
                centerX: editor.centerX
                centerY: editor.centerY
                zoom: editor.zoom
                // The draft shows the first composited frame. Save applies the
                // same crop to every frame and preserves exact animation timing.
                playing: false
                MouseArea {
                    anchors.fill: parent
                    enabled: !editor.operationBusy
                    cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                    property real previousX: 0
                    property real previousY: 0
                    onPressed: mouse => {
                        previousX = mouse.x;
                        previousY = mouse.y;
                        forceActiveFocus();
                    }
                    onPositionChanged: mouse => {
                        if (!pressed)
                            return;
                        editor.centerX -= (mouse.x - previousX) / width * editor.side / editor.draft.width;
                        editor.centerY -= (mouse.y - previousY) / height * editor.side / editor.draft.height;
                        previousX = mouse.x;
                        previousY = mouse.y;
                        editor.constrain();
                    }
                    activeFocusOnTab: true
                    Accessible.name: "Crop position; use arrow keys to move the picture"
                    Keys.onLeftPressed: { editor.centerX -= 0.02; editor.constrain(); }
                    Keys.onRightPressed: { editor.centerX += 0.02; editor.constrain(); }
                    Keys.onUpPressed: { editor.centerY -= 0.02; editor.constrain(); }
                    Keys.onDownPressed: { editor.centerY += 0.02; editor.constrain(); }
                }
            }
            CordsLabel {
                text: "Zoom · " + editor.zoom.toFixed(1) + "×"
            }
            Slider {
                id: zoomControl
                objectName: "avatarCropZoom"
                from: 1
                to: 6
                value: 1
                enabled: !editor.operationBusy
                Layout.fillWidth: true
                Accessible.name: "Picture crop zoom"
                onValueChanged: editor.constrain()
            }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                CordsButton { text: "Circle"; selected: editor.shape === "circle"; enabled: !editor.operationBusy; onClicked: editor.shape = "circle" }
                CordsButton { text: "Square"; selected: editor.shape === "square"; enabled: !editor.operationBusy; onClicked: editor.shape = "square" }
                CordsButton { text: "Reset crop"; enabled: !editor.operationBusy; onClicked: editor.resetCrop() }
                CordsButton { text: "Choose another picture…"; enabled: !editor.operationBusy; onClicked: editor.chooseRequested() }
            }
            CordsLabel {
                text: "Animations keep all frames; this preview shows the first frame. Nothing is published until you save."
                color: Theme.secondary
                Layout.fillWidth: true
            }
            CordsLabel {
                text: editor.errorText || cropPreview.imageError
                visible: text.length > 0
                color: Theme.danger
                Layout.fillWidth: true
            }
        }
    }
    footer: RowLayout {
        spacing: 8
        CordsButton {
            text: "Cancel"
            enabled: !editor.operationBusy
            onClicked: editor.close()
        }
        Item { Layout.fillWidth: true }
        CordsButton {
            objectName: "saveAvatarCrop"
            text: editor.operationBusy ? "Working…" : "Save picture"
            primary: true
            enabled: !editor.operationBusy && !!editor.draft.id
            onClicked: editor.saveRequested(editor.draft.id, editor.centerX, editor.centerY, editor.zoom, editor.shape)
        }
    }
}
