import QtQuick

CordsButton {
    id: control
    property string iconName: ""
    quiet: true
    implicitWidth: Theme.controlHeight
    implicitHeight: Theme.controlHeight
    help: text
    contentItem: CordsIcon {
        name: control.iconName
        color: control.primary ? Theme.textOnAccent : control.destructive ? Theme.danger : Theme.text
        opacity: control.enabled ? 1 : 0.55
    }
}
