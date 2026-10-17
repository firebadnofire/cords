#include "jxlimage.h"
#include <QFile>
#include <QFileInfo>
#include <QDir>
#include <QGuiApplication>
#include <QQuickWindow>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QTest>
#include <QThreadPool>
#include <vector>

class MediaTest : public QObject {
    Q_OBJECT
private slots:
    void avatarPreferencesAndExplicitPlayback() {
        qmlRegisterType<JxlImage>("Cords", 1, 0, "JxlImage");
        const auto qmlPath = QStringLiteral(CORDS_QML_DIR);
        qmlRegisterSingletonType(QUrl::fromLocalFile(qmlPath + "/Theme.qml"), "Cords", 1, 0, "Theme");
        QQmlEngine engine;
        auto *theme = engine.singletonInstance<QObject *>(qmlTypeId("Cords", 1, 0, "Theme"));
        QVERIFY(theme);
        QVERIFY(theme->setProperty("disableAnimations", true));
        QQmlComponent component(&engine, QUrl::fromLocalFile(qmlPath + "/CordsAvatar.qml"));
        QScopedPointer<QObject> avatar(component.create());
        QVERIFY2(avatar, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(avatar.data());
        auto *image = avatar->findChild<JxlImage *>("avatarPlayback");
        QVERIFY(item && image);
        QQuickWindow window;
        window.resize(160, 160);
        item->setParentItem(window.contentItem());
        item->setSize({100, 100});
        QFile fixture(QStringLiteral(CORDS_TEST_IMAGE));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        QVERIFY(avatar->setProperty("source", QUrl(QString::fromLatin1("data:image/jxl;base64," + fixture.readAll().toBase64()))));
        window.show();
        QVERIFY(!image->playing());
        QTRY_VERIFY_WITH_TIMEOUT(image->ready() || !image->error().isEmpty(), 90000);
        QVERIFY2(image->error().isEmpty(), qPrintable(image->error()));
        const auto still = image->frameNumber();
        QTest::qWait(700);
        QCOMPARE(image->frameNumber(), still);
        QVERIFY(theme->setProperty("disableAnimations", false));
        QVERIFY(theme->setProperty("reducedMotion", true));
        QVERIFY(!image->playing());
        QVERIFY(avatar->setProperty("playing", true));
        QVERIFY(avatar->setProperty("motionOverride", true));
        QVERIFY(image->playing());
        QTRY_VERIFY_WITH_TIMEOUT(image->frameNumber() > still, 10000);
        window.hide();
        QTest::qWait(350);
        const auto hidden = image->frameNumber();
        QTest::qWait(700);
        QCOMPARE(image->frameNumber(), hidden);
        window.show();
        QTRY_VERIFY_WITH_TIMEOUT(image->frameNumber() > hidden, 10000);
        QVERIFY(avatar->setProperty("playing", false));
        QVERIFY(!image->playing());
        const auto paused = image->frameNumber();
        QTest::qWait(700);
        QCOMPARE(image->frameNumber(), paused);
        QVERIFY(avatar->setProperty("motionOverride", false));
        QVERIFY(avatar->setProperty("playing", true));
        QVERIFY(!image->playing());
        QVERIFY(theme->setProperty("reducedMotion", false));
        QVERIFY(image->playing());
        item->setParentItem(nullptr);
    }
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
    void repeatedAvatarsHaveIndependentTimelines() {
        QFile fixture(QStringLiteral(CORDS_TEST_IMAGE));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        const auto source = "data:image/jxl;base64," + fixture.readAll().toBase64();
        QQuickWindow window;
        window.resize(420, 160);
        JxlImage first(window.contentItem()), second(window.contentItem());
        for (auto *view : {&first, &second}) {
            view->setSize({100, 100}); view->setPlaying(false);
            view->setAccount("same-account"); view->setSource(source);
        }
        second.setX(110); window.show();
        QTRY_VERIFY_WITH_TIMEOUT(first.ready() && second.ready(), 90000);
        QCOMPARE(first.m_playback, second.m_playback);
        const auto still = first.m_image;
        first.setPlaying(true);
        QTRY_VERIFY_WITH_TIMEOUT(first.frameNumber() >= 3, 15000);
        QCOMPARE(second.m_image, still);
        second.setPlaying(true);
        QVERIFY(first.m_playback != second.m_playback);
        QTRY_VERIFY_WITH_TIMEOUT(second.frameNumber() >= 2, 15000);
        first.setPlaying(false);
        QCOMPARE(first.m_image, still);
        const auto frame = second.frameNumber();
        QTRY_VERIFY_WITH_TIMEOUT(second.frameNumber() > frame, 15000);
        QCOMPARE(first.m_image, still);
        const auto playback = second.m_playback;
        second.setPlaying(true); second.setSquare(true);
        QCOMPARE(second.m_playback, playback);
        second.setPlaying(false);
        QCOMPARE(first.m_playback, second.m_playback);
        QCOMPARE(second.m_image, still);
    }
    void finiteLoopStopsAndCanBeTriggeredAgain() {
        QFile fixture(QFileInfo(QStringLiteral(CORDS_TEST_IMAGE)).dir().filePath("finite.jxl"));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        QQuickWindow window; window.resize(160, 160);
        JxlImage image(window.contentItem()); image.setSize({100, 100});
        image.setSource("data:image/jxl;base64," + fixture.readAll().toBase64()); window.show();
        QTRY_COMPARE_WITH_TIMEOUT(image.frameNumber(), 4, 15000);
        QVERIFY(image.animated());
        QTest::qWait(1200);
        QCOMPARE(image.frameNumber(), 4);
        image.setPlaying(false); QCOMPARE(image.frameNumber(), 1);
        image.setPlaying(true);
        QTRY_COMPARE_WITH_TIMEOUT(image.frameNumber(), 4, 15000);
    }
    void staticJxlNeverStartsAnAnimationTimeline() {
        QFile fixture(QFileInfo(QStringLiteral(CORDS_TEST_IMAGE)).dir().filePath("static.jxl"));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        QQuickWindow window; window.resize(160, 160);
        JxlImage image(window.contentItem()); image.setSize({100, 100});
        image.setPlaying(false);
        image.setSource("data:image/jxl;base64," + fixture.readAll().toBase64()); window.show();
        QTRY_VERIFY_WITH_TIMEOUT(image.ready(), 15000);
        QVERIFY(!image.animated());
        const auto still = image.m_playback;
        const auto pixels = image.m_image;
        image.setPlaying(true);
        QTest::qWait(700);
        QCOMPARE(image.m_playback, still);
        QCOMPARE(image.m_image, pixels);
        QCOMPARE(image.frameNumber(), 1);
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
