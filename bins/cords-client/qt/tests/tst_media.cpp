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
    void repeatedAccountAvatarsShareFramesAndLifetime() {
        QFile fixture(QStringLiteral(CORDS_TEST_IMAGE));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        const auto source = "data:image/jxl;base64," + fixture.readAll().toBase64();
        QQuickWindow window;
        window.resize(420, 160);
        window.show();
        std::vector<std::unique_ptr<JxlImage>> avatars;
        // More views than the distinct-image budget must still use one entry.
        for (int index = 0; index < 80; ++index) {
            auto view = std::make_unique<JxlImage>(window.contentItem());
            view->setSize({20, 20});
            view->setPosition({qreal(index % 20 * 20), qreal(index / 20 * 20)});
            view->setAccount("verified-account-a"); view->setSource(source);
            avatars.push_back(std::move(view));
        }
        auto *first = avatars.front().get();
        auto *copy = avatars.back().get();
        QTRY_VERIFY_WITH_TIMEOUT(first->frameNumber() >= 3 && copy->ready(), 90000);
        for (const auto &view : avatars) {
            QVERIFY2(view->error().isEmpty(), qPrintable(view->error()));
            QCOMPARE(view->m_playback.get(), first->m_playback.get());
            QCOMPARE(view->frameNumber(), first->frameNumber());
            QCOMPARE(view->m_image.cacheKey(), first->m_image.cacheKey());
        }
        copy->setPlaying(false);
        const auto paused = copy->frameNumber();
        QTRY_VERIFY_WITH_TIMEOUT(first->frameNumber() > paused, 10000);
        QCOMPARE(copy->frameNumber(), paused);
        copy->setPlaying(true);
        QCOMPARE(copy->frameNumber(), first->frameNumber());
        copy->setVisible(false);
        const auto hidden = copy->frameNumber();
        QTRY_VERIFY_WITH_TIMEOUT(first->frameNumber() > hidden, 10000);
        QCOMPARE(copy->frameNumber(), hidden);
        copy->setVisible(true);
        QCOMPARE(copy->frameNumber(), first->frameNumber());

        // Identity, image revision, and crop/mask have different jobs.
        copy->setAccount("verified-account-b");
        QVERIFY(copy->m_playback.get() != first->m_playback.get());
        QTRY_VERIFY_WITH_TIMEOUT(copy->ready(), 15000);
        copy->setAccount("verified-account-a");
        QCOMPARE(copy->m_playback.get(), first->m_playback.get());
        copy->setSquare(true); copy->setSize({70, 70});
        QCOMPARE(copy->m_image.cacheKey(), first->m_image.cacheKey());
        QFile replacement(QFileInfo(QStringLiteral(CORDS_TEST_IMAGE)).dir().filePath("looping-b.jxl"));
        QVERIFY(replacement.open(QIODevice::ReadOnly));
        copy->setSource("data:image/jxl;base64," + replacement.readAll().toBase64());
        QVERIFY(copy->m_playback.get() != first->m_playback.get());
        QTRY_VERIFY_WITH_TIMEOUT(copy->ready(), 15000);
        QVERIFY(copy->m_image.cacheKey() != first->m_image.cacheKey());
        copy->setSource(source);
        QCOMPARE(copy->m_playback.get(), first->m_playback.get());

        window.hide(); QTest::qWait(350);
        const auto stopped = first->frameNumber();
        QTest::qWait(700);
        QCOMPARE(first->frameNumber(), stopped);
        window.show();
        QTRY_VERIFY_WITH_TIMEOUT(first->frameNumber() > stopped, 15000);
        const std::weak_ptr<JxlPlayback> released = first->m_playback;
        avatars.clear();
        QVERIFY(released.expired());
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
