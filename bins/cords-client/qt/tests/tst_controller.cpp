#include "controller.h"
#include "platformevents.h"
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
