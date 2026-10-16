#include "jxlimage.h"
#include <QFile>
#include <QGuiApplication>
#include <QQuickWindow>
#include <QTest>
#include <QThreadPool>

class MediaTest : public QObject {
    Q_OBJECT
private slots:
    void nativePlaybackAndHiddenCleanup() {
        QFile fixture(QStringLiteral(CORDS_TEST_IMAGE));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        QQuickWindow window;
        window.resize(160, 160);
        JxlImage image(window.contentItem());
        image.setSize({128, 128});
        image.setSource("data:image/jxl;base64," + fixture.readAll().toBase64());
        window.show();
        QTRY_VERIFY_WITH_TIMEOUT(image.ready() || !image.error().isEmpty(), 90000);
        QVERIFY2(image.error().isEmpty(), qPrintable(image.error()));
        const auto first = image.frameNumber();
        QTest::qWait(200);
        QCOMPARE(image.frameNumber(), first);
        QTRY_VERIFY_WITH_TIMEOUT(image.frameNumber() >= 3, 10000);
        const auto frame = window.grabWindow();
        QVERIFY(!frame.isNull());
        QVERIFY(frame.save(QStringLiteral(CORDS_TEST_CAPTURE)));
        image.setPlaying(false);
        const auto stopped = image.frameNumber();
        QTest::qWait(700);
        QCOMPARE(image.frameNumber(), stopped);
        image.setPlaying(true);
        QTRY_VERIFY_WITH_TIMEOUT(image.frameNumber() > stopped, 10000);
        window.hide();
        const auto hidden = image.frameNumber();
        QTest::qWait(700);
        QCOMPARE(image.frameNumber(), hidden);
        // A destroyed view may have work in flight; jobs retain only their Rust
        // decoder, never a pointer to the destroyed Qt item.
    }
    void boundedSlotsYieldToOtherVisibleImages() {
        QFile fixture(QStringLiteral(CORDS_TEST_IMAGE));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        const auto source = "data:image/jxl;base64," + fixture.readAll().toBase64();
        QQuickWindow window;
        window.resize(420, 160);
        JxlImage first(window.contentItem()), second(window.contentItem()), third(window.contentItem());
        for (auto *image : {&first, &second, &third}) { image->setSize({100, 100}); image->setSource(source); }
        second.setX(110); third.setX(220); window.show();
        QTRY_VERIFY_WITH_TIMEOUT(first.frameNumber() >= 2 && second.frameNumber() >= 2 && third.frameNumber() >= 2, 15000);
        QVERIFY(first.error().isEmpty() && second.error().isEmpty() && third.error().isEmpty());
        third.setX(1000);
        QTest::qWait(350);
        const auto offscreen = third.frameNumber();
        QTest::qWait(700);
        QCOMPARE(third.frameNumber(), offscreen);
        third.setX(220);
        QTRY_VERIFY_WITH_TIMEOUT(third.frameNumber() > offscreen, 15000);
    }
    void cleanupTestCase() { QThreadPool::globalInstance()->waitForDone(); }
};
QTEST_MAIN(MediaTest)
#include "tst_media.moc"
