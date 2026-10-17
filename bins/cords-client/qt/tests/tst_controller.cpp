#include "controller.h"
#include "platformevents.h"
#include "jxlimage.h"
#include <QQmlComponent>
#include <QQmlEngine>
#include <QFile>
#include <QThreadPool>
#include <QQuickWindow>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QTest>
#ifdef Q_OS_WIN
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#endif

class ControllerTest : public QObject {
    Q_OBJECT
private slots:
    void initTestCase() {
        qmlRegisterUncreatableType<Controller>("Cords", 1, 0, "Controller", "Test owned");
        qmlRegisterUncreatableType<Rows>("Cords", 1, 0, "Rows", "Test owned");
        qmlRegisterType<JxlImage>("Cords", 1, 0, "JxlImage");
        const auto qmlPath = QStringLiteral(CORDS_QML_DIR);
        qmlRegisterSingletonType(QUrl::fromLocalFile(qmlPath + "/Theme.qml"), "Cords", 1, 0, "Theme");
    }
    void productionQmlHoverProfilesAndLayouts() {
        const auto qmlPath = QStringLiteral(CORDS_QML_DIR);
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        QFile fixture(QStringLiteral(CORDS_TEST_IMAGE));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        const auto imageData = "data:image/jxl;base64," + fixture.readAll().toBase64();
        // Presentation-only fixture: no account/server or delivery claim.
        const QVariantMap person{{"image", QString::fromLatin1(imageData)}, {"shape", "circle"}};
        controller.m_state = {{"account", "local-test"}, {"route", "test-channel"},
            {"origin", "https://presentation.invalid"}, {"directoryLoaded", true},
            {"people", QVariantMap{{"test-person", person}}},
            {"profile", QVariantMap{{"account", "test-person"}, {"name", "Test person"}, {"image", QString::fromLatin1(imageData)}}}};
        controller.m_busy = true; // prevents fixture UI from submitting operations
        controller.channels()->replace({{{"id", "test-channel"}, {"name", "general"}, {"mode", "encrypted"}, {"joined", true}}});
        controller.messages()->replace({{{"id", "test-message"}, {"account", "test-person"}, {"author", "Test person"}, {"body", "Local presentation fixture. Hover anywhere in this message to play its avatar."}, {"timestamp", 1700000000}}});
        controller.members()->replace({{{"id", "test-person"}, {"name", "Test person"}, {"group", "Members"}}});
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(qmlPath + "/Shell.qml"));
        QScopedPointer<QObject> shell(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(shell, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(shell.data());
        QVERIFY(item);
        QQuickWindow window;
        window.setColor(QColor("#313338"));
        window.resize(1180, 760); item->setParentItem(window.contentItem()); item->setSize(window.size());
        window.show();
        emit controller.stateChanged();
        QTest::qWait(100);
        const std::function<QQuickItem *(QQuickItem *, const QString &)> findVisible = [&findVisible](QQuickItem *root, const QString &name) -> QQuickItem * {
            if (root->objectName() == name && root->isVisible()) return root;
            for (auto *child : root->childItems()) if (auto *found = findVisible(child, name)) return found;
            return nullptr;
        };
        auto *message = findVisible(item, "messageRow");
        auto *member = findVisible(item, "memberRow");
        QVERIFY(message && member);
        auto *messageImage = message->findChild<JxlImage *>("avatarPlayback");
        auto *memberImage = member->findChild<JxlImage *>("avatarPlayback");
        QVERIFY(messageImage && memberImage);
        QTRY_VERIFY_WITH_TIMEOUT(messageImage->ready() && memberImage->ready(), 90000);
        QVERIFY(!messageImage->playing() && !memberImage->playing());
        QTest::mouseMove(&window, message->mapToScene(QPointF(message->width() - 20, message->height() / 2)).toPoint());
        QTRY_VERIFY(messageImage->playing());
        QTRY_VERIFY_WITH_TIMEOUT(messageImage->frameNumber() >= 2, 15000);
        QVERIFY(!memberImage->playing());
        QTest::mouseMove(&window, member->mapToScene(QPointF(80, member->height() / 2)).toPoint());
        QTRY_VERIFY(memberImage->playing());
        QTRY_VERIFY(!messageImage->playing());
        QCOMPARE(messageImage->frameNumber(), 1);
        QTRY_VERIFY_WITH_TIMEOUT(memberImage->frameNumber() >= 2, 15000);
        QTest::mouseMove(&window, QPoint(600, 10));
        QTRY_VERIFY(!memberImage->playing());
        QCOMPARE(memberImage->frameNumber(), 1);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-wide.png"));
        auto *profile = item->findChild<QObject *>("expandedProfile");
        QVERIFY(profile);
        profile->setProperty("account", "test-person");
        QVERIFY(QMetaObject::invokeMethod(profile, "open"));
        auto *profileImage = profile->findChild<JxlImage *>("avatarPlayback");
        QVERIFY(profileImage);
        QTRY_VERIFY(profileImage->playing());
        QTRY_VERIFY_WITH_TIMEOUT(profileImage->frameNumber() >= 2, 15000);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-profile.png"));
        QVERIFY(QMetaObject::invokeMethod(profile, "close"));
        QTRY_VERIFY(!profileImage->playing());
        window.resize(680, 520); item->setSize(window.size());
        QTest::qWait(150);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-narrow.png"));
        QVERIFY(QMetaObject::invokeMethod(profile, "open"));
        QTest::qWait(200);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-profile-narrow.png"));
        QVERIFY(QMetaObject::invokeMethod(profile, "close"));
        item->setProperty("showSettings", true);
        auto *settings = item->findChild<QObject *>("accountSettings");
        QVERIFY(settings);
        settings->setProperty("section", "Privacy");
        QTest::qWait(150);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-settings-narrow.png"));
        window.resize(1180, 760); item->setSize(window.size());
        settings->setProperty("section", "Appearance");
        QTest::qWait(150);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-settings.png"));
        item->setParentItem(nullptr);
        QThreadPool::globalInstance()->waitForDone();
    }
    void accountPickerUsesRealVaultAuthentication() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AccountPicker.qml"));
        QScopedPointer<QObject> picker(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(picker, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(picker.data());
        QVERIFY(item);
        QQuickWindow window; window.resize(1180, 760);
        item->setParentItem(window.contentItem()); item->setSize(window.size()); window.show();
        auto *password = picker->findChild<QQuickItem *>("accountPassword");
        auto *submit = picker->findChild<QQuickItem *>("accountSubmit");
        auto *add = picker->findChild<QQuickItem *>("addAccountTile");
        auto *nickname = picker->findChild<QObject *>("accountNickname");
        QVERIFY(password && submit && add && nickname);
        QVERIFY(!password->isVisible() && !submit->isVisible());
        QTest::qWait(100);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/picker-empty.png"));
        QTest::mouseClick(&window, Qt::LeftButton, Qt::NoModifier, add->mapToScene(QPointF(60, 60)).toPoint());
        QTRY_VERIFY(password->isVisible());
        nickname->setProperty("text", "First local identity");
        password->setProperty("text", "Mango Lantern selector acceptance 83");
        auto *confirmation = picker->findChild<QObject *>("accountConfirmation");
        QVERIFY(confirmation);
        confirmation->setProperty("text", "Mango Lantern selector acceptance 83");
        QVERIFY(submit->isEnabled());
        QVERIFY(QMetaObject::invokeMethod(submit, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 10000);
        QVERIFY2(controller.state().value("unlocked").toBool(), qPrintable(controller.state().value("error").toString()));
        const auto first = controller.state().value("account").toString();
        QCOMPARE(password->property("text").toString(), QString());
        controller.lockAccount(false);
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 10000);
        QVERIFY(QMetaObject::invokeMethod(picker.data(), "resetSelection"));
        QVERIFY(!password->isVisible() && !submit->isVisible());
        QCOMPARE(controller.accounts()->rowCount(), 1);
        QVERIFY(QMetaObject::invokeMethod(picker.data(), "selectIdentity", Q_ARG(QVariant, first), Q_ARG(QVariant, QString("First local identity"))));
        QTRY_VERIFY(password->hasActiveFocus());
        QVERIFY(!controller.state().value("unlocked").toBool());
        password->setProperty("text", "wrong password");
        QTest::keyClick(&window, Qt::Key_Return);
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 10000);
        QVERIFY(!controller.state().value("error").toString().isEmpty());
        QCOMPARE(picker->property("selectedAccount").toString(), first);
        QVERIFY(password->isVisible());
        password->setProperty("text", "Mango Lantern selector acceptance 83");
        QTest::keyClick(&window, Qt::Key_Return);
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 10000);
        QVERIFY(controller.state().value("unlocked").toBool());
        controller.lockAccount(false); QTRY_VERIFY(!controller.busy());
        controller.createAccount("Second local identity", "Mango Lantern selector second 27", false);
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 10000);
        QVERIFY(controller.state().value("unlocked").toBool());
        const auto second = controller.state().value("account").toString();
        QVERIFY(first != second);
        controller.lockAccount(false); QTRY_VERIFY(!controller.busy());
        QVERIFY(QMetaObject::invokeMethod(picker.data(), "resetSelection"));
        QCOMPARE(controller.accounts()->rowCount(), 2);
        QTest::qWait(100);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/picker-multiple.png"));
        QVERIFY(QMetaObject::invokeMethod(picker.data(), "selectIdentity", Q_ARG(QVariant, second), Q_ARG(QVariant, QString("Second local identity"))));
        QTRY_VERIFY(password->hasActiveFocus());
        window.resize(680, 520); item->setSize(window.size()); QTest::qWait(150);
        QTRY_VERIFY(submit->mapToScene(QPointF(0, submit->height())).y() <= window.height());
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/picker-selected-narrow.png"));
        QTest::keyClick(&window, Qt::Key_Escape);
        QTRY_VERIFY(!password->isVisible() && !submit->isVisible());
        QCOMPARE(picker->property("selectedAccount").toString(), QString());
        item->setParentItem(nullptr);
    }
    void historyRowsKeepPersistentAnchorsAcrossPrependAndPrune() {
        Rows rows;
        const auto page = [](int first, int count) {
            QList<QVariantMap> result;
            for (int index = first; index < first + count; ++index)
                result.append({{"id", QString::number(index)}, {"body", QStringLiteral("Message %1").arg(index)}});
            return result;
        };
        rows.replace(page(200, 100));
        QPersistentModelIndex anchor(rows.index(25));
        rows.replace(page(100, 200));
        QVERIFY(anchor.isValid());
        QCOMPARE(anchor.row(), 125);
        QCOMPARE(anchor.data(Qt::UserRole).toMap().value("id").toString(), "225");
        rows.replace(page(0, 300));
        QCOMPARE(anchor.row(), 225);
        rows.replace(page(100, 300));
        QCOMPARE(anchor.row(), 125);
        auto changed = page(100, 300);
        changed[125]["body"] = "Updated presentation";
        rows.replace(changed);
        QVERIFY(anchor.isValid());
        QCOMPARE(anchor.data(Qt::UserRole).toMap().value("body").toString(), "Updated presentation");
        QCOMPARE(rows.indexOf("225"), 125);
        rows.replace(page(400, 100));
        QVERIFY(!anchor.isValid());
        rows.replace({});
        QCOMPARE(rows.rowCount(), 0);
    }
    void pendingNetworkLockClearsViewAndReopensVault() {
        QTemporaryDir state;
        QVERIFY(state.isValid());
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.createAccount("Native controller", "Mango Lantern interrupted TLS 57", false);
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 10000);
        QVERIFY2(controller.state().value("unlocked").toBool(), qPrintable(controller.state().value("error").toString()));
        const auto account = controller.state().value("account").toString();
        QTcpServer server;
        QVERIFY(server.listen(QHostAddress::LocalHost));
        controller.trustServer(QStringLiteral("https://127.0.0.1:%1").arg(server.serverPort()));
        QTRY_VERIFY_WITH_TIMEOUT(server.hasPendingConnections(), 5000);
        auto *peer = server.nextPendingConnection();
        QVERIFY(peer);
        QQuickWindow window;
        PlatformEvents events(controller, window);
#ifdef Q_OS_WIN
        MSG event{};
        event.hwnd = reinterpret_cast<HWND>(window.winId());
        event.message = WM_WTSSESSION_CHANGE;
        event.wParam = WTS_SESSION_LOCK;
        qintptr result = 0;
        QVERIFY(!events.nativeEventFilter("windows_generic_MSG", &event, &result));
#else
        controller.platformLock(false);
#endif
        // Presentation disappears synchronously, before the Rust completion.
        QVERIFY(!controller.state().value("unlocked").toBool());
        QCOMPARE(controller.messages()->rowCount(), 0);
        QCOMPARE(controller.servers()->rowCount(), 0);
        QCOMPARE(controller.members()->rowCount(), 0);
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 2000);
        QVERIFY2(controller.state().value("error").toString().isEmpty(), qPrintable(controller.state().value("error").toString()));
        QCOMPARE(controller.accounts()->rowCount(), 1);
        controller.unlockAccount(account, "Mango Lantern interrupted TLS 57");
        QTRY_VERIFY_WITH_TIMEOUT(!controller.busy(), 10000);
        QVERIFY(controller.state().value("unlocked").toBool());
        QCOMPARE(controller.state().value("account").toString(), account);
        peer->close();
    }
};
QTEST_MAIN(ControllerTest)
#include "tst_controller.moc"
