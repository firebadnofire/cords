import QtQuick
import QtQuick.Controls.Basic

CheckBox {
    id: control
    spacing: 10
    implicitHeight: Math.max(Theme.controlHeight, contentItem.implicitHeight + topPadding + bottomPadding)
    Accessible.name: text
    hoverEnabled: true
    indicator: Rectangle {
        implicitWidth: 20
        implicitHeight: 20
        x: control.leftPadding
        y: (control.height - height) / 2
        color: control.checked ? Theme.accent : control.hovered ? Theme.hover : Theme.input
        border.width: control.visualFocus ? 2 : 1
        border.color: control.visualFocus ? Theme.focus : Theme.controlBorder
        radius: 4
        opacity: control.enabled ? 1 : 0.55
        CordsIcon {
            anchors.centerIn: parent
            name: "check"
            visible: control.checked
            color: Theme.textOnAccent
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
