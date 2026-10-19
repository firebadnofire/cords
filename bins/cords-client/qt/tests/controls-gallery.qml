import QtQuick
import QtQuick.Layouts
import "../qml" as Cords

Rectangle {
    id: gallery
    property string appearance: "dark"
    property real textScale: 1
    readonly property int effectiveBodySize: Cords.Theme.bodySize
    onAppearanceChanged: Cords.Theme.appearance = appearance
    onTextScaleChanged: Cords.Theme.textScale = textScale
    color: Cords.Theme.conversation
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 16
        Cords.CordsLabel {
            text: "Native component acceptance"
            font.pixelSize: Cords.Theme.headingSize
        }
        Cords.CordsLabel {
            text: "Presentation fixture · no account or server data"
            color: Cords.Theme.secondary
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            Cords.CordsButton {
                text: "Continue"
                primary: true
            }
            Cords.CordsButton {
                text: "Selected"
                selected: true
            }
            Cords.CordsButton {
                text: "Remove"
                destructive: true
            }
            Cords.CordsButton {
                text: "Unavailable"
                enabled: false
            }
            Cords.CordsButton {
                text: "Save"
                busy: true
            }
            Cords.CordsIconButton {
                text: "Refresh requests"
                iconName: "refresh"
            }
        }
        Cords.CordsField {
            objectName: "galleryField"
            Layout.fillWidth: true
            text: "Editable community name"
            Accessible.name: "Community name"
        }
        Cords.CordsField {
            Layout.fillWidth: true
            text: "Unsaved value"
            errorText: "Couldn't save. Check the connection and retry."
            Accessible.name: "Failed save"
        }
        Cords.CordsLabel {
            text: "Couldn't save. Check the connection and retry."
            color: Cords.Theme.danger
            Layout.fillWidth: true
        }
        Cords.CordsCheck {
            text: "Reduce motion"
            checked: true
        }
        Cords.CordsButton {
            objectName: "galleryMenuButton"
            text: "Actions"
            onClicked: actions.popup()
        }
        Item {
            Layout.fillHeight: true
        }
    }
    Cords.CordsMenu {
        id: actions
        objectName: "galleryMenu"
        Cords.CordsMenuItem {
            text: "Copy identity"
        }
        Cords.CordsMenuItem {
            text: "Unavailable operation"
            enabled: false
        }
    }
}
