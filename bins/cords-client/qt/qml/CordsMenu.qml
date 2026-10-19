import QtQuick
import QtQuick.Controls.Basic

Menu {
    id: control
    readonly property real widestItem: {
        let result = 168;
        for (let index = 0; index < count; ++index) {
            const item = itemAt(index);
            if (item)
                result = Math.max(result, item.implicitWidth);
        }
        return result;
    }
    implicitWidth: widestItem + leftPadding + rightPadding
    padding: 6
    margins: 8
    font.pixelSize: Theme.bodySize
    palette.windowText: Theme.text
    palette.text: Theme.text
    palette.highlight: Theme.selected
    palette.highlightedText: Theme.text
    background: Rectangle {
        color: Theme.elevated
        radius: Theme.popoverRadius
        border.color: Theme.divider
    }
    delegate: CordsMenuItem { }
}
