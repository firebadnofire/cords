import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: control
    implicitHeight: Theme.controlHeight
    color: Theme.text
    placeholderTextColor: Theme.muted
    selectionColor: Theme.accent
    selectedTextColor: Theme.onAccent
    font.pixelSize: Theme.bodySize
    padding: 10
    selectByMouse: true
    Accessible.name: placeholderText
    background: Rectangle {
        color: Theme.rail
        radius: Theme.radius
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? Theme.focus : Theme.border
    }
}
