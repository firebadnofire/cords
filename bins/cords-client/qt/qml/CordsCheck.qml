import QtQuick
import QtQuick.Controls.Basic

CheckBox {
    id: control
    spacing: 10
    implicitHeight: Theme.controlHeight
    Accessible.name: text
    indicator: Rectangle {
        implicitWidth: 20
        implicitHeight: 20
        x: control.leftPadding
        y: (control.height - height) / 2
        color: control.checked ? Theme.accent : Theme.rail
        border.width: control.visualFocus ? 2 : 1
        border.color: control.visualFocus ? Theme.focus : Theme.border
        radius: 4
        Text {
            anchors.centerIn: parent
            text: control.checked ? "✓" : ""
            color: Theme.onAccent
        }
    }
    contentItem: Text {
        text: control.text
        textFormat: Text.PlainText
        font.pixelSize: Theme.bodySize
        color: control.enabled ? Theme.text : Theme.muted
        leftPadding: control.indicator.width + control.spacing
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
    }
}
