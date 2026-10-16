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
}
