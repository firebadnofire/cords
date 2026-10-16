import QtQuick

// Keep visible message IDs stable while a bounded model prepends or prunes rows.
ListView {
    id: view
    property bool followTail: true
    property bool anchorPending: false
    property string anchorId: ""
    property string anchorScope: ""
    property real anchorOffset: 0
    property string historyScope: ""
    property bool browsing: false
    function rememberViewport() {
        if (anchorPending)
            return;
        anchorPending = true;
        anchorId = "";
        anchorScope = historyScope;
        if (followTail)
            return;
        for (let offset = 1; offset < Math.min(height, 64); offset += 4) {
            const index = indexAt(contentX + leftMargin + 1, contentY + offset);
            const item = itemAtIndex(index);
            if (index >= 0 && item) {
                anchorId = model.get(index).id;
                anchorOffset = item.y - contentY;
                break;
            }
        }
    }
    function restoreViewport() {
        forceLayout();
        if (followTail) {
            positionViewAtEnd();
            forceLayout();
            contentY = Math.max(originY, originY + contentHeight - height);
        } else if (anchorId && anchorScope === historyScope) {
            const index = model.indexOf(anchorId);
            if (index >= 0) {
                positionViewAtIndex(index, ListView.Beginning);
                forceLayout();
                const item = itemAtIndex(index);
                if (item)
                    contentY = item.y - anchorOffset;
            } else {
                positionViewAtBeginning();
            }
        }
        anchorPending = false;
    }
    onHistoryScopeChanged: {
        anchorId = "";
        followTail = true;
        Qt.callLater(restoreViewport);
    }
    onBrowsingChanged: if (!browsing) {
        followTail = true;
        Qt.callLater(restoreViewport);
    }
    onMovementStarted: followTail = false
    onMovementEnded: followTail = atYEnd && !browsing
    Component.onCompleted: Qt.callLater(restoreViewport)
    Connections {
        target: view.model
        function onAboutToChange() { view.rememberViewport(); }
        function onChanged() { Qt.callLater(view.restoreViewport); }
    }
    onContentHeightChanged: if (followTail) Qt.callLater(restoreViewport)
    onHeightChanged: if (followTail) Qt.callLater(restoreViewport)
}
