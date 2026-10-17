import QtQuick
import QtQuick.Controls.Basic

Button {
    id: control
    property bool primary: false
    property bool quiet: false
    property bool selected: false
    property bool destructive: false
    property string help: ""
    implicitHeight: Theme.controlHeight
    implicitWidth: Math.max(72, contentItem.implicitWidth + 24)
    padding: 10
    focusPolicy: Qt.StrongFocus
    hoverEnabled: true
    font.pixelSize: Theme.bodySize
    Accessible.name: text
    Accessible.description: help
    contentItem: Text {
        text: control.text
        textFormat: Text.PlainText
        font: control.font
        color: control.primary && control.enabled ? Theme.onAccent : control.destructive ? Theme.danger : Theme.text
        horizontalAlignment: control.quiet ? Text.AlignLeft : Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1 : 0.55
    }
    background: Rectangle {
        radius: Theme.radius
        color: control.primary && control.enabled ? Theme.accent : control.down ? Theme.border : control.selected ? Theme.selected : control.hovered ? Theme.hover : control.quiet ? "transparent" : Theme.elevated
        border.width: control.visualFocus ? 2 : control.destructive ? 1 : 0
        border.color: control.visualFocus ? Theme.focus : control.destructive ? Theme.danger : Theme.border
        opacity: control.enabled ? 1 : 0.6
    }
    // A local Basic tooltip has an explicit QML dependency. Attached ToolTip
    // dynamically imports the generic Controls module outside deployment scans.
    ToolTip {
        objectName: "buttonHelp"
        visible: control.hovered && control.help.length > 0
        text: control.help
        delay: 500
    }
}
