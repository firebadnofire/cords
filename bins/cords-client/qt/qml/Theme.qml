pragma Singleton
import QtQuick

QtObject {
    property string appearance: "dark"
    property bool compact: false
    property bool reducedMotion: false
    property bool disableAnimations: false
    readonly property bool light: appearance === "light"
    readonly property color window: light ? "#edf0f3" : appearance === "midnight" ? "#0c1016" : "#252e38"
    readonly property color rail: light ? "#d6dce3" : "#131c25"
    readonly property color navigation: light ? "#e0e5eb" : "#1d2630"
    readonly property color conversation: window
    readonly property color selected: light ? "#cbd6e2" : "#364655"
    readonly property color elevated: light ? "#ffffff" : "#303d4a"
    readonly property color hover: light ? "#cbd6e2" : "#303e4c"
    readonly property color accent: light ? "#38678f" : "#8dc4e7"
    readonly property color onAccent: light ? "#ffffff" : "#101a24"
    readonly property color text: light ? "#1b2733" : "#f2f3f5"
    readonly property color secondary: light ? "#465969" : "#b5bac4"
    readonly property color muted: light ? "#57697a" : "#a3a8b2"
    readonly property color border: light ? "#b8c6d3" : "#3b4a59"
    readonly property color danger: light ? "#a52c40" : "#ef9aa8"
    readonly property color warning: light ? "#805617" : "#e3c38b"
    readonly property color focus: light ? "#275e87" : "#a3d2f2"
    readonly property int spacing: compact ? 6 : 8
    readonly property int controlHeight: compact ? 30 : 36
    readonly property int rowHeight: compact ? 32 : 38
    readonly property int radius: 5
    readonly property int bodySize: 14
    readonly property int captionSize: 12
    readonly property int headingSize: 22
}
