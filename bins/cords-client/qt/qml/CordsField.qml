import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: control
    property string errorText: ""
    property string help: ""
    implicitHeight: Math.max(Theme.controlHeight, contentHeight + 16)
    color: Theme.text
    placeholderTextColor: Theme.muted
    selectionColor: Theme.accent
    selectedTextColor: Theme.textOnAccent
    font.pixelSize: Theme.bodySize
    padding: 10
    selectByMouse: true
    Accessible.name: placeholderText
    Accessible.description: errorText || help
    opacity: enabled ? 1 : 0.6
    background: Rectangle {
        color: Theme.input
        radius: Theme.radius
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? Theme.focus : control.errorText ? Theme.danger : Theme.controlBorder
    }
}
