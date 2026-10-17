import QtQuick
import QtTest
import "../qml" as Cords

TestCase {
    name: "ServerClaim"
    when: windowShown
    visible: true
    width: 680
    height: 720
    Cords.ServerClaim { id: claim }
    SignalSpy { id: requests; target: claim; signalName: "claimRequested" }
    function init() {
        claim.server = "";
        claim.ownership = "";
        claim.operationBusy = false;
        requests.clear();
    }
    function cleanup() {
        claim.server = "";
        tryCompare(claim, "opened", false);
    }
    function test_entry_dismiss_reopen_and_switch() {
        claim.operationBusy = true;
        claim.server = "first";
        claim.ownership = "UNCLAIMED";
        verify(!claim.opened);
        claim.operationBusy = false;
        tryCompare(claim, "opened", true);
        const code = findChild(claim, "serverClaimCode");
        code.text = "synthetic-code";
        mouseClick(findChild(claim, "dismissServerClaim"));
        tryCompare(claim, "opened", false);
        compare(code.text, "");
        claim.operationBusy = true;
        claim.operationBusy = false;
        verify(!claim.opened);
        claim.open();
        tryCompare(claim, "opened", true);
        code.text = "synthetic-code";
        mouseClick(findChild(claim, "submitServerClaim"));
        compare(requests.count, 1);
        compare(requests.signalArguments[0][0], "synthetic-code");
        compare(code.text, "");
        claim.errorText = "Invalid claim code";
        verify(claim.opened);
        claim.close();
        tryCompare(claim, "opened", false);
        claim.server = "second";
        tryCompare(claim, "opened", true);
        claim.ownership = "CLAIMED";
        tryCompare(claim, "opened", false);
    }
}
