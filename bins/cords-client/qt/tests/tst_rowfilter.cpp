#include "rows.h"
#include "rowfilter.h"
#include <QAbstractItemModelTester>
#include <QTest>

class RowFilterTest : public QObject {
    Q_OBJECT
private slots:
    void filteringSortingAndUpdates() {
        Rows rows;
        RowFilter filter;
        filter.setSourceModel(&rows);
        QAbstractItemModelTester sourceCheck(&rows, QAbstractItemModelTester::FailureReportingMode::QtTest);
        QAbstractItemModelTester proxyCheck(&filter, QAbstractItemModelTester::FailureReportingMode::QtTest);
        rows.replace({{{"id", "b"}, {"name", "Zulu"}, {"roles", QStringList{"moderator"}}, {"priority", 10}},
                      {{"id", "a"}, {"name", "alice"}, {"roles", QStringList{}}, {"priority", 2}}});
        QCOMPARE(filter.get(0).value("id").toString(), "a");
        filter.setQuery(" ZUL ");
        QCOMPARE(filter.rowCount(), 1);
        QCOMPARE(filter.get(0).value("id").toString(), "b");
        filter.setQuery("moderator");
        QCOMPARE(filter.rowCount(), 1);
        filter.setRoleFilter("absent");
        QCOMPARE(filter.rowCount(), 0);
        filter.setRoleFilter(""); filter.setQuery(""); filter.setSortField("priority");
        QCOMPARE(filter.get(0).value("id").toString(), "a");
        filter.setDescending(true);
        QCOMPARE(filter.get(0).value("id").toString(), "b");
        filter.setQuery("new");
        rows.replace({{{"id", "b"}, {"name", "new name"}, {"priority", 1}}});
        QCOMPARE(filter.rowCount(), 1);
        rows.replace({});
        QCOMPARE(filter.rowCount(), 0);
        QVERIFY(filter.get(-1).isEmpty());
    }
};
QTEST_GUILESS_MAIN(RowFilterTest)
#include "tst_rowfilter.moc"
