import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: view
    property string title: ""
    property string explanation: ""
    property string actionText: ""
    property bool busy: false
    signal actionRequested()
    spacing: 12
    CordsLabel {
        text: view.title
        font.pixelSize: Theme.headingSize
        font.weight: Font.DemiBold
        horizontalAlignment: Text.AlignHCenter
        Layout.fillWidth: true
        Accessible.role: Accessible.Heading
    }
    CordsLabel {
        text: view.explanation
        color: Theme.secondary
        horizontalAlignment: Text.AlignHCenter
        Layout.fillWidth: true
    }
    CordsButton {
        objectName: "emptyStateAction"
        text: view.actionText
        visible: !!text
        busy: view.busy
        primary: true
        Layout.alignment: Qt.AlignHCenter
        onClicked: view.actionRequested()
    }
}
