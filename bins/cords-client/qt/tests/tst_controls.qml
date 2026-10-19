import QtQuick
import QtTest
import "../qml" as Cords

TestCase {
    name: "CordsControls"
    when: windowShown
    visible: true
    width: 480
    height: 320
    Cords.CordsButton { id: button; x: 20; y: 20; text: "Send message"; property int activations: 0; onClicked: activations++ }
    Cords.CordsField { id: field; x: 20; y: 90; placeholderText: "Message" }
    Cords.CordsCheck { id: check; x: 20; y: 150; text: "Reduced motion" }
    Cords.CordsIconButton { id: iconButton; x: 300; y: 20; iconName: "refresh"; text: "Refresh requests"; property int activations: 0; onClicked: activations++ }
    Cords.CordsMenu {
        id: menu
        property int selections: 0
        Cords.CordsMenuItem { text: "Copy identity"; onTriggered: menu.selections++ }
    }
    function test_keyboard_and_disabled_action() {
        button.forceActiveFocus()
        keyClick(Qt.Key_Space)
        compare(button.activations, 1)
        button.enabled = false
        mouseClick(button)
        compare(button.activations, 1)
        button.enabled = true
    }
    function test_text_focus_and_toggle() {
        field.forceActiveFocus()
        keyClick(Qt.Key_N)
        keyClick(Qt.Key_A)
        compare(field.text, "na")
        check.forceActiveFocus()
        keyClick(Qt.Key_Space)
        verify(check.checked)
    }
    function test_explicit_tooltip_can_show_and_hide() {
        button.help = "Native button help"
        const tip = findChild(button, "buttonHelp")
        verify(tip !== null)
        mouseMove(button, button.width / 2, button.height / 2)
        tryCompare(tip, "opened", true, 1500)
        compare(tip.text, button.help)
        mouseMove(field, 2, 2)
        tryCompare(tip, "opened", false, 1500)
        button.help = ""
    }
    function test_busy_error_and_icon_keyboard() {
        iconButton.forceActiveFocus()
        keyClick(Qt.Key_Space)
        compare(iconButton.activations, 1)
        iconButton.busy = true
        verify(!iconButton.enabled)
        mouseClick(iconButton)
        compare(iconButton.activations, 1)
        iconButton.busy = false
        verify(iconButton.enabled)
        field.errorText = "Couldn't save. Try again."
        compare(field.Accessible.description, field.errorText)
        field.errorText = ""
    }
    function test_context_menu_keyboard_and_escape() {
        menu.popup(30, 30)
        tryCompare(menu, "opened", true)
        keyClick(Qt.Key_Down)
        keyClick(Qt.Key_Return)
        tryCompare(menu, "selections", 1)
        tryCompare(menu, "opened", false)
        menu.popup(30, 30)
        tryCompare(menu, "opened", true)
        keyClick(Qt.Key_Escape)
        tryCompare(menu, "opened", false)
    }
    function test_semantic_text_contrast() {
        const previous = Cords.Theme.appearance
        function luminance(color) {
            function linear(value) { return value <= 0.04045 ? value / 12.92 : Math.pow((value + 0.055) / 1.055, 2.4) }
            return linear(color.r) * 0.2126 + linear(color.g) * 0.7152 + linear(color.b) * 0.0722
        }
        function ratio(a, b) {
            const x = luminance(a), y = luminance(b)
            return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
        }
        try {
            for (const theme of ["dark", "midnight", "light"]) {
                Cords.Theme.appearance = theme
                for (const surface of ["input", "conversation", "elevated", "selected", "hover", "navigation"]) {
                    for (const text of ["textPrimary", "textSecondary", "textMuted", "danger", "success", "warning"]) {
                        verify(ratio(Cords.Theme[text], Cords.Theme[surface]) >= 4.5, theme + ": " + text + " on " + surface)
                    }
                }
                verify(ratio(Cords.Theme.textOnAccent, Cords.Theme.accent) >= 4.5, theme + ": primary action " + Cords.Theme.textOnAccent + " / " + Cords.Theme.accent)
                verify(ratio(Cords.Theme.controlBorder, Cords.Theme.input) >= 3, theme + ": input boundary")
            }
        } finally {
            Cords.Theme.appearance = previous
        }
    }
}
