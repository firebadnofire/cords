import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CordsDialog {
    id: dialog
    property string explanation: ""
    property string confirmText: "Confirm"
    property var operation: null
    ColumnLayout {
        width: parent.width
        spacing: 16
        CordsLabel {
            text: dialog.explanation
            Layout.fillWidth: true
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            CordsButton {
                text: "Cancel"
                onClicked: dialog.close()
            }
            CordsButton {
                text: dialog.confirmText
                destructive: true
                onClicked: {
                    dialog.close();
                    if (dialog.operation)
                        dialog.operation();
                }
            }
        }
    }
}
