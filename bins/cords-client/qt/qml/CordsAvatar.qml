import QtQuick
import Cords

Rectangle {
    id: avatar
    property string name: ""
    property string account: ""
    property string shape: "circle"
    property url source: ""
    property bool playing: true
    property bool motionOverride: false
    property real centerX: 0.5
    property real centerY: 0.5
    property real zoom: 1
    readonly property string imageError: nativeImage.error
    width: 36
    height: width
    radius: shape === "square" ? 5 : width / 2
    color: Theme.elevated
    border.color: Theme.border
    clip: true
    Accessible.name: name
    Text {
        anchors.centerIn: parent
        text: avatar.name.slice(0, 2).toUpperCase()
        color: Theme.accent
        font.pixelSize: avatar.width * 0.38
        visible: !nativeImage.ready
    }
    JxlImage {
        id: nativeImage
        objectName: "avatarPlayback"
        account: avatar.account
        anchors.fill: parent
        source: avatar.source.toString().startsWith("data:image/jxl;base64,") || avatar.source.toString().startsWith("data:image/png;base64,") ? avatar.source.toString() : ""
        playing: avatar.playing && (avatar.motionOverride || (!Theme.reducedMotion && !Theme.disableAnimations))
        square: avatar.shape === "square"
        centerX: avatar.centerX
        centerY: avatar.centerY
        zoom: avatar.zoom
    }
}
