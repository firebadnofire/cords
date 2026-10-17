import QtQuick
import QtQuick.Controls.Basic

Dialog {
    id: control
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(500, parent ? parent.width - 32 : 500)
    padding: 20
    font.pixelSize: Theme.bodySize
    palette.windowText: Theme.text
    palette.text: Theme.text
    palette.buttonText: Theme.text
    palette.button: Theme.elevated
    closePolicy: Popup.CloseOnEscape
    header: CordsLabel {
        text: control.title
        font.pixelSize: Theme.headingSize
        padding: 20
        bottomPadding: 8
    }
    background: Rectangle {
        color: Theme.navigation
        border.color: Theme.border
        radius: Theme.radius
    }
}
