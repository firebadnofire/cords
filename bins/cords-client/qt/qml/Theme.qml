pragma Singleton
import QtQuick

QtObject {
    property string appearance: "dark"
    property bool compact: false
    property bool reducedMotion: false
    property bool disableAnimations: false
    property real textScale: 1
    property int serverIconSize: 48
    readonly property bool light: appearance === "light"
    readonly property bool midnight: appearance === "midnight"
    // HIG 2: semantic surfaces. Legacy aliases keep existing callers on one theme.
    readonly property color window: light ? "#edf0f3" : midnight ? "#111216" : "#232429"
    readonly property color serverRail: light ? "#d6dce3" : midnight ? "#15161b" : "#202127"
    readonly property color rail: serverRail
    readonly property color navigation: light ? "#e0e5eb" : midnight ? "#1b1d23" : "#2b2d33"
    readonly property color memberRail: navigation
    readonly property color conversation: light ? "#ffffff" : midnight ? "#202228" : "#32343b"
    readonly property color input: light ? "#f7f8fa" : midnight ? "#15171c" : "#262830"
    readonly property color selected: light ? "#cbd6e2" : "#454954"
    readonly property color elevated: light ? "#ffffff" : midnight ? "#292c34" : "#3b3d46"
    readonly property color hover: light ? "#d4dce6" : "#3e414a"
    readonly property color divider: light ? "#c1c9d3" : "#42454d"
    readonly property color border: divider
    readonly property color controlBorder: light ? "#7a8491" : "#858b98"
    readonly property color accent: light ? "#286387" : "#83bdd9"
    readonly property color textOnAccent: light ? "#ffffff" : "#17232b"
    readonly property color textPrimary: light ? "#1b2733" : "#f2f3f5"
    readonly property color textSecondary: light ? "#465360" : "#c6c9d0"
    readonly property color textMuted: light ? "#505b68" : "#b7bcc6"
    readonly property color text: textPrimary
    readonly property color secondary: textSecondary
    readonly property color muted: textMuted
    readonly property color danger: light ? "#a52c40" : "#ffa3a8"
    readonly property color success: light ? "#216237" : "#78c99a"
    readonly property color warning: light ? "#714a11" : "#e8bc70"
    readonly property color focus: light ? "#275e87" : "#a3d2f2"
    readonly property int spacing: compact ? 6 : 8
    readonly property int controlHeight: Math.max(compact ? 32 : 36, bodySize + 16)
    readonly property int rowHeight: Math.max(compact ? 32 : 36, bodySize + 16)
    readonly property int memberRowHeight: Math.max(compact ? 38 : 44, bodySize + 20)
    readonly property int headerHeight: Math.max(50, bodySize + 28)
    readonly property int radius: 7
    readonly property int popoverRadius: 10
    readonly property int bodySize: Math.round(14 * textScale)
    readonly property int captionSize: Math.round(12 * textScale)
    readonly property int headingSize: Math.round(22 * textScale)
    readonly property int motionDuration: reducedMotion ? 0 : 120
}
