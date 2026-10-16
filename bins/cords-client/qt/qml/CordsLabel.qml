import QtQuick
import QtQuick.Controls.Basic

Label {
    color: Theme.text
    font.pixelSize: Theme.bodySize
    textFormat: Text.PlainText
    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
    Accessible.name: text
}
