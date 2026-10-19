import QtQuick
import QtTest
import "../qml" as Cords

TestCase {
    id: test
    name: "LocalAccountRemoval"
    when: windowShown
    visible: true
    width: 680
    height: 520
    Cords.LocalAccountRemoval {
        id: removal
        account: "synthetic-local-account"
        nickname: "Disposable fixture"
    }
    SignalSpy {
        id: requests
        target: removal
        signalName: "removalRequested"
    }
    function init() {
        requests.clear();
        removal.operationBusy = false;
        removal.open();
        tryCompare(removal, "opened", true);
    }
    function cleanup() {
        removal.operationBusy = false;
        removal.close();
        tryCompare(removal, "opened", false);
    }
    function test_confirmation_targets_one_account_and_clears_password() {
        const password = findChild(removal, "localRemovalPassword");
        const confirm = findChild(removal, "confirmRemoval");
        verify(password !== null && confirm !== null);
        verify(confirm.enabled);
        password.text = "public-test-only-local-password";
        verify(confirm.enabled);
        mouseClick(confirm);
        compare(requests.count, 1);
        compare(requests.signalArguments[0][0], "synthetic-local-account");
        compare(requests.signalArguments[0][1], "public-test-only-local-password");
        compare(password.text, "");
        verify(confirm.enabled);
        verify(removal.width <= test.width - 32);
        verify(removal.height <= test.height - 32);
    }
    function test_cancel_clears_input_and_busy_blocks_submission() {
        const password = findChild(removal, "localRemovalPassword");
        const confirm = findChild(removal, "confirmRemoval");
        const cancel = findChild(removal, "cancelRemoval");
        password.text = "public-test-only-local-password";
        removal.operationBusy = true;
        verify(!password.enabled && !confirm.enabled && !cancel.enabled);
        mouseClick(confirm);
        compare(requests.count, 0);
        removal.operationBusy = false;
        mouseClick(cancel);
        tryCompare(removal, "opened", false);
        compare(password.text, "");
        compare(requests.count, 0);
    }
    function test_empty_password_is_submitted_for_vault_authentication() {
        const password = findChild(removal, "localRemovalPassword");
        const confirm = findChild(removal, "confirmRemoval");
        compare(password.text, "");
        verify(confirm.enabled);
        mouseClick(confirm);
        compare(requests.count, 1);
        compare(requests.signalArguments[0][0], "synthetic-local-account");
        compare(requests.signalArguments[0][1], "");
        removal.account = "";
        verify(!confirm.enabled);
        removal.account = "synthetic-local-account";
    }
}
