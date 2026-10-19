import QtQuick
import QtQuick.Controls.Basic

MenuItem {
    id: control
    implicitWidth: Math.max(160, contentItem.implicitWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(Theme.controlHeight, contentItem.implicitHeight + 12)
    font.pixelSize: Theme.bodySize
    leftPadding: 12
    rightPadding: 12
    contentItem: Text {
        text: control.text
        textFormat: Text.PlainText
        font: control.font
        color: Theme.text
        opacity: control.enabled ? 1 : 0.55
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: Theme.radius
        color: control.highlighted ? Theme.selected : "transparent"
        border.width: control.visualFocus ? 2 : 0
        border.color: Theme.focus
    }
}
