pragma Singleton
import QtQuick

QtObject {
    property string appearance: "dark"
    property bool compact: false
    property bool reducedMotion: false
    readonly property bool light: appearance === "light"
    readonly property color window: light ? "#edf0f3" : appearance === "midnight" ? "#0c1016" : "#1a202a"
    readonly property color rail: light ? "#d6dce3" : "#111720"
    readonly property color navigation: light ? "#e0e5eb" : "#171e28"
    readonly property color conversation: window
    readonly property color elevated: light ? "#ffffff" : "#242e3c"
    readonly property color hover: light ? "#cbd6e2" : "#2c394a"
    readonly property color accent: light ? "#38678f" : "#80b5db"
    readonly property color onAccent: light ? "#ffffff" : "#101a24"
    readonly property color text: light ? "#1b2733" : "#e4eaf1"
    readonly property color secondary: light ? "#465969" : "#a7b6c7"
    readonly property color muted: light ? "#57697a" : "#8393a6"
    readonly property color border: light ? "#b8c6d3" : "#354356"
    readonly property color danger: light ? "#a52c40" : "#ef9aa8"
    readonly property color warning: light ? "#805617" : "#e3c38b"
    readonly property color focus: light ? "#275e87" : "#a3d2f2"
    readonly property int spacing: compact ? 6 : 8
    readonly property int controlHeight: compact ? 32 : 38
    readonly property int rowHeight: compact ? 34 : 42
    readonly property int radius: 7
    readonly property int bodySize: 14
    readonly property int captionSize: 12
    readonly property int headingSize: 22
}
