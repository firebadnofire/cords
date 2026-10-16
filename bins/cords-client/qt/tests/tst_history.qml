import QtQuick
import QtTest
import "../qml" as Cords

TestCase {
    name: "HistoryViewport"
    when: windowShown
    width: 480
    height: 320
    ListModel {
        id: rows
        signal aboutToChange()
        signal changed()
        function indexOf(id) {
            for (let index = 0; index < count; ++index)
                if (get(index).id === id) return index;
            return -1;
        }
    }
    Cords.HistoryView {
        id: history
        width: 480
        height: 320
        model: rows
        spacing: 8
        leftMargin: 14
        historyScope: "fixture/account/server/channel"
        delegate: Rectangle {
            required property string id
            width: history.width - 28
            height: 40 + Number(id) % 5 * 11
            Text { text: parent.id }
        }
    }
    function init() {
        rows.aboutToChange();
        rows.clear();
        for (let index = 200; index < 300; ++index) rows.append({id: String(index)});
        rows.changed();
        history.browsing = false;
        history.followTail = true;
        history.restoreViewport();
        wait(80);
    }
    function test_prepend_retains_visible_id_and_offset() {
        history.browsing = true;
        history.followTail = false;
        history.positionViewAtIndex(65, ListView.Beginning);
        history.forceLayout();
        history.contentY += 17;
        wait(40);
        history.rememberViewport();
        const anchor = history.anchorId;
        const offset = history.anchorOffset;
        verify(anchor.length > 0);
        rows.aboutToChange();
        for (let index = 199; index >= 100; --index) rows.insert(0, {id: String(index)});
        rows.changed();
        wait(80);
        const item = history.itemAtIndex(rows.indexOf(anchor));
        verify(item !== null);
        fuzzyCompare(item.y - history.contentY, offset, 1);
        rows.aboutToChange();
        for (let index = 99; index >= 0; --index) rows.insert(0, {id: String(index)});
        rows.changed();
        wait(80);
        fuzzyCompare(history.itemAtIndex(rows.indexOf(anchor)).y - history.contentY, offset, 1);
    }
    function test_pruned_anchor_and_return_to_latest() {
        history.browsing = true;
        history.followTail = false;
        history.positionViewAtIndex(90, ListView.Beginning);
        wait(40);
        rows.aboutToChange();
        rows.remove(0, rows.count);
        for (let index = 0; index < 100; ++index) rows.append({id: String(index)});
        rows.changed();
        wait(80);
        verify(history.atYBeginning);
        history.browsing = false;
        wait(80);
        verify(history.atYEnd, JSON.stringify({y: history.contentY, origin: history.originY, content: history.contentHeight, height: history.height, tail: history.followTail}));
        history.height = 240;
        wait(80);
        verify(history.atYEnd);
        history.height = 320;
    }
}
