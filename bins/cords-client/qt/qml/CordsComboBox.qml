import QtQuick
import QtQuick.Controls.Basic

ComboBox {
    id: control
    implicitHeight: Theme.controlHeight
    font.pixelSize: Theme.bodySize
    palette.text: Theme.text
    palette.buttonText: Theme.text
    palette.windowText: Theme.text
    palette.base: Theme.rail
    palette.window: Theme.navigation
    palette.button: Theme.elevated
    palette.highlight: Theme.selected
    palette.highlightedText: Theme.text
    background: Rectangle {
        color: control.hovered ? Theme.hover : Theme.rail
        radius: Theme.radius
        border.color: control.visualFocus ? Theme.focus : Theme.border
        border.width: control.visualFocus ? 2 : 1
    }
}
