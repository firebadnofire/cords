pragma ComponentBehavior: Bound
import Cords
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtCore

ApplicationWindow {
    id: window
    required property Controller desktop
    width: settings.windowWidth
    height: settings.windowHeight
    minimumWidth: 680
    minimumHeight: 520
    visible: true
    title: "Cords — Right on the wire"
    color: Theme.window
    font.pixelSize: Theme.bodySize
    header: CordsLabel {
        text: window.desktop.state.platformWarning || ""
        visible: text.length > 0
        padding: 8
        color: Theme.warning
    }
    Settings {
        id: settings
        property int windowWidth: 1180
        property int windowHeight: 760
    }
    onClosing: {
        if (visibility !== Window.Maximized) {
            settings.windowWidth = width;
            settings.windowHeight = height;
        }
    }
    Connections {
        target: window.desktop
        function onStateChanged() {
            const prefs = window.desktop.state.preferences;
            if (prefs) {
                Theme.appearance = prefs.theme;
                Theme.compact = prefs.compact;
                Theme.reducedMotion = prefs.reducedMotion;
            }
        }
    }
    Shortcut {
        sequence: "Ctrl+L"
        enabled: !!window.desktop.state.unlocked
        onActivated: window.desktop.lockAccount(false)
    }
    Loader {
        anchors.fill: parent
        sourceComponent: window.desktop.state.unlocked ? shell : picker
    }
    Component {
        id: picker
        AccountPicker {
            desktop: window.desktop
        }
    }
    Component {
        id: shell
        Shell {
            desktop: window.desktop
        }
    }
}
