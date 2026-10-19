import QtQuick
import QtQuick.Controls.Basic

Button {
    id: control
    property bool primary: false
    property bool quiet: false
    property bool selected: false
    property bool destructive: false
    property string help: ""
    property bool busy: false
    property string errorText: ""
    property bool succeeded: false
    enabled: !busy
    implicitHeight: Math.max(Theme.controlHeight, contentItem.implicitHeight + 12)
    implicitWidth: Math.max(72, contentItem.implicitWidth + 24)
    padding: 8
    focusPolicy: Qt.StrongFocus
    hoverEnabled: true
    font.pixelSize: Theme.bodySize
    Accessible.name: text
    Accessible.description: errorText || (busy ? "Working" : help)
    contentItem: Text {
        text: control.busy ? "Working…" : control.text
        textFormat: Text.PlainText
        font: control.font
        color: control.primary && control.enabled ? Theme.textOnAccent : control.destructive ? Theme.danger : Theme.text
        horizontalAlignment: control.quiet ? Text.AlignLeft : Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1 : 0.55
    }
    background: Rectangle {
        radius: Theme.radius
        color: control.primary && control.enabled ? Theme.accent : control.down ? Theme.border : control.selected ? Theme.selected : control.hovered ? Theme.hover : control.quiet ? "transparent" : Theme.elevated
        border.width: control.visualFocus ? 2 : control.destructive || control.errorText || control.succeeded ? 1 : 0
        border.color: control.visualFocus ? Theme.focus : control.destructive || control.errorText ? Theme.danger : control.succeeded ? Theme.success : Theme.border
        opacity: control.enabled ? 1 : 0.6
    }
    // A local Basic tooltip has an explicit QML dependency. Attached ToolTip
    // dynamically imports the generic Controls module outside deployment scans.
    ToolTip {
        objectName: "buttonHelp"
        visible: (control.hovered || control.visualFocus) && text.length > 0
        text: control.errorText || control.help
        delay: 500
        palette.toolTipBase: Theme.elevated
        palette.toolTipText: Theme.text
    }
}
