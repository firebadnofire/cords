import QtQuick
import QtQuick.Controls.Basic

Button {
    id: control
    property bool primary: false
    property bool destructive: false
    property string help: ""
    implicitHeight: Theme.controlHeight
    implicitWidth: Math.max(72, contentItem.implicitWidth + 24)
    padding: 10
    focusPolicy: Qt.StrongFocus
    Accessible.name: text
    Accessible.description: help
    contentItem: Text {
        text: control.text
        textFormat: Text.PlainText
        font: control.font
        color: control.primary && control.enabled ? Theme.onAccent : control.destructive ? Theme.danger : Theme.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1 : 0.55
    }
    background: Rectangle {
        radius: Theme.radius
        color: control.primary && control.enabled ? Theme.accent : control.down ? Theme.border : control.hovered ? Theme.hover : Theme.elevated
        border.width: control.visualFocus ? 2 : 1
        border.color: control.visualFocus ? Theme.focus : control.destructive ? Theme.danger : Theme.border
        opacity: control.enabled ? 1 : 0.6
    }
    ToolTip.visible: hovered && help.length > 0
    ToolTip.text: help
}
