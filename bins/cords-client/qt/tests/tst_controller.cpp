#include "controller.h"
#include "rowfilter.h"
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
#include <QSignalSpy>
#include <QJsonDocument>
#include <QJsonObject>
#ifdef Q_OS_WIN
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#endif

class ControllerTest : public QObject {
    Q_OBJECT
private slots:
    void initTestCase() {
#ifdef Q_OS_WIN
        QGuiApplication::setFont(QFont(QStringLiteral("Segoe UI")));
#endif
        qmlRegisterUncreatableType<Controller>("Cords", 1, 0, "Controller", "Test owned");
        qmlRegisterUncreatableType<Rows>("Cords", 1, 0, "Rows", "Test owned");
        qmlRegisterType<RowFilter>("Cords", 1, 0, "RowFilter");
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
        controller.m_state = {{"account", "local-test"}, {"route", "test-channel"}, {"admission", "active"},
            {"origin", "https://presentation.invalid"}, {"directoryLoaded", true}, {"canApprove", true}, {"requestsLoaded", true}, {"serverProfile", QVariantMap{{"supported", true}, {"loaded", true}, {"canEdit", true}, {"revision", 3}, {"name", "Cords community"}, {"description", "A signed community profile with durable server-owned presentation."}}},
            {"people", QVariantMap{{"test-person", person}}},
            {"profile", QVariantMap{{"account", "test-person"}, {"name", "Test person"}, {"image", QString::fromLatin1(imageData)}}}};
        controller.m_busy = true; // prevents fixture UI from submitting operations
        auto serverProfile = controller.m_state.value("serverProfile").toMap();
        serverProfile.insert("image", QString::fromLatin1(imageData));
        serverProfile.insert("shape", "circle");
        serverProfile.insert("hasIcon", true);
        controller.m_state.insert("serverProfile", serverProfile);
        controller.servers()->replace({{{"id", "test-server"}, {"name", "Cords community"}, {"origin", "https://presentation.invalid"}, {"selected", true}, {"state", "active"}, {"image", QString::fromLatin1(imageData)}, {"shape", "circle"}}});
        controller.channels()->replace({{{"id", "test-channel"}, {"name", "general"}, {"mode", "encrypted"}, {"joined", true}}});
        controller.messages()->replace({
            {{"id", "test-message"}, {"account", "test-person"}, {"author", "Test person"}, {"body", "Local presentation fixture. Hover anywhere in this message to play its avatar."}, {"timestamp", 1700000000}},
            {{"id", "followup"}, {"account", "test-person"}, {"author", "Test person"}, {"body", "A grouped follow-up from the same verified account."}, {"timestamp", 1700000060}},
            {{"id", "other-account"}, {"account", "other-test-person"}, {"author", "Test person"}, {"body", "The same display name belongs to a different account: begin a new group."}, {"timestamp", 1700000120}}});
        controller.requests()->replace({{{"id", "verified-device"}, {"account", "verified-account"}, {"name", "Applicant"}, {"status", "pending"}, {"requestedAt", 1700000000}}});
        controller.members()->replace({{{"id", "test-person"}, {"name", "Test person"}, {"group", "Members"}, {"roles", QStringList{"fixture-role"}}}});
        controller.roles()->replace({{{"id", "fixture-role"}, {"name", "Community guides"}, {"color", "#8dc4e7"}}});
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(qmlPath + "/Shell.qml"));
        QQuickWindow window;
        QScopedPointer<QObject> shell(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(shell, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(shell.data());
        QVERIFY(item);
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
        QQuickItem *message = nullptr;
        int groupedRows = 0;
        const std::function<void(QQuickItem *)> countGroups = [&countGroups, &groupedRows, &message](QQuickItem *root) {
            if (root->objectName() == "messageRow") {
                if (root->property("grouped").toBool()) ++groupedRows;
                if (root->property("row").toMap().value("id") == "test-message") message = root;
            }
            for (auto *child : root->childItems()) countGroups(child);
        };
        countGroups(item);
        qInfo() << "Rendered message grouping count" << groupedRows;
        QCOMPARE(groupedRows, 1);
        QVERIFY(message && !message->property("grouped").toBool());
        auto *serverIcon = findVisible(item, "serverRailIcon");
        QVERIFY(serverIcon);
        auto *serverImage = serverIcon->findChild<JxlImage *>("avatarPlayback");
        QVERIFY(serverImage);
        QTRY_VERIFY_WITH_TIMEOUT(serverImage->ready(), 90000);
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
        auto *card = item->findChild<QObject *>("accountCard");
        QVERIFY(card);
        QVERIFY(QMetaObject::invokeMethod(card, "openFor", Q_ARG(QVariant, "test-person"), Q_ARG(QVariant, QVariant::fromValue(member))));
        QTRY_VERIFY(card->property("opened").toBool());
        QVERIFY(!card->property("modal").toBool());
        QVERIFY(card->property("x").toReal() >= 12 && card->property("x").toReal() + card->property("width").toReal() <= window.width() - 12);
        auto *cardAvatar = card->findChild<QObject *>("accountCardAvatar");
        QVERIFY(cardAvatar);
        QTRY_VERIFY(cardAvatar->property("playing").toBool());
        QTest::qWait(150);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-account-card.png"));
        auto *details = card->findChild<QQuickItem *>("accountCardDetails");
        QVERIFY(details);
        QTest::mouseClick(&window, Qt::LeftButton, Qt::NoModifier, details->mapToScene(QPointF(details->width() / 2, details->height() / 2)).toPoint());
        QTRY_VERIFY(card->property("details").toBool());
        // A real profile refresh can reset the directory while the card is open.
        // Restore to the current account row rather than a deleted delegate.
        controller.members()->replace({{{"id", "test-person"}, {"name", "Test person"}, {"group", "Members"}, {"roles", QStringList{"fixture-role"}}}});
        QTest::qWait(100);
        member = findVisible(item, "memberRow");
        QVERIFY(member);
        QTest::keyClick(&window, Qt::Key_Escape);
        QTRY_VERIFY(!card->property("opened").toBool());
        QTRY_VERIFY(member->hasActiveFocus());
        auto *search = item->findChild<QQuickItem *>("messageSearch");
        auto *emptySearch = item->findChild<QQuickItem *>("conversationEmptyState");
        QVERIFY(search && emptySearch);
        QTest::keyClick(&window, Qt::Key_F, Qt::ControlModifier);
        QTRY_VERIFY(search->hasActiveFocus());
        search->setProperty("text", "no-such-loaded-body-token");
        QTRY_VERIFY(emptySearch->isVisible());
        QCOMPARE(emptySearch->property("title").toString(), QString("No loaded messages match"));
        QTest::keyClick(&window, Qt::Key_Escape);
        QTRY_COMPARE(search->property("text").toString(), QString());
        QTRY_VERIFY(!emptySearch->isVisible());
        message->forceActiveFocus();
        QTest::keyClick(&window, Qt::Key_Menu);
        auto *messageMenu = message->findChild<QObject *>("messageContextMenu");
        QVERIFY(messageMenu);
        QTRY_VERIFY(messageMenu->property("opened").toBool());
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-fixture-message-menu.png"));
        QTest::keyClick(&window, Qt::Key_Escape);
        QTRY_VERIFY(!messageMenu->property("opened").toBool());
        auto *profile = item->findChild<QObject *>("expandedProfile");
        QVERIFY(profile);
        profile->setProperty("account", "test-person");
        QVERIFY(QMetaObject::invokeMethod(profile, "open"));
        auto *profileAvatar = profile->findChild<QObject *>("publicProfileAvatar");
        QVERIFY(profileAvatar);
        auto *profileImage = profileAvatar->findChild<JxlImage *>("avatarPlayback");
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
        QVERIFY(settings->property("width").toReal() > 1100);
        QSignalSpy closeRequested(settings, SIGNAL(closeRequested()));
        QVERIFY(QMetaObject::invokeMethod(settings, "change", Q_ARG(QVariant, "name"), Q_ARG(QVariant, "Unsaved nickname")));
        QVERIFY(QMetaObject::invokeMethod(settings, "requestClose"));
        QTRY_VERIFY(settings->property("modalOpen").toBool());
        QCOMPARE(closeRequested.count(), 0);
        auto *discard = settings->findChild<QObject *>("discardPreferences");
        QVERIFY(discard);
        QVERIFY(QMetaObject::invokeMethod(discard, "close"));
        QVERIFY(settings->property("dirty").toBool());
        QVERIFY(QMetaObject::invokeMethod(settings, "reload"));
        QVERIFY(QMetaObject::invokeMethod(settings, "requestClose"));
        QCOMPARE(closeRequested.count(), 1);
        item->setProperty("showSettings", false);
        item->setProperty("showAdmin", true);
        auto *admin = item->findChild<QObject *>("serverSettings");
        QVERIFY(admin);
        controller.roles()->replace({{{"id", "fixture-role"}, {"name", "Community guides"}, {"color", "#8dc4e7"}, {"priority", 1}}});
        for (const auto size : {QSize(1920, 1080), QSize(680, 520)}) {
            window.resize(size); item->setSize(size);
            for (int section : {0, 1, 2, 3, 6, 7}) {
                admin->setProperty("page", section);
                QTest::qWait(100);
                QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT)
                    + QString("/ui-fixture-admin-%1-%2.png").arg(section).arg(size.width())));
            }
            QVERIFY(admin->property("width").toReal() >= size.width() - 1);
        }
        admin->setProperty("page", 0);
        controller.m_state["serverIconDraft"] = QVariantMap{{"id", "server-icon-fixture"}, {"image", QString::fromLatin1(imageData)}, {"width", 100}, {"height", 100}};
        emit controller.stateChanged();
        auto *iconEditor = admin->findChild<QObject *>("serverIconEditor");
        QVERIFY(iconEditor);
        QTRY_VERIFY(iconEditor->property("visible").toBool());
        QVERIFY(iconEditor->property("height").toReal() <= window.height() - 16);
        QVERIFY(admin->property("modalOpen").toBool());
        controller.m_state.remove("serverIconDraft");
        emit controller.stateChanged();
        QTRY_VERIFY(!iconEditor->property("visible").toBool());
        QTest::keyClick(&window, Qt::Key_Escape);
        QTRY_VERIFY(!item->property("showAdmin").toBool());
        item->setParentItem(nullptr);
        QThreadPool::globalInstance()->waitForDone();
    }
    void composerSeparatesDraftScopesAndExplainsDisabledSending() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        // Presentation-only input test; this does not claim a server authorized sending.
        controller.m_state = {{"account", "account-a"}, {"server", "server-a"}, {"route", "channel-a"}, {"admission", "active"}, {"connected", true}};
        controller.channels()->replace({{{"id", "channel-a"}, {"name", "general"}, {"mode", "public"}, {"joined", true}}, {{"id", "channel-b"}, {"name", "other"}, {"mode", "encrypted"}, {"joined", false}}});
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/Shell.qml"));
        QQuickWindow window;
        QScopedPointer<QObject> shell(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(shell, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(shell.data());
        QVERIFY(item);
        window.resize(1024, 768); item->setParentItem(window.contentItem()); item->setSize(window.size()); window.show();
        emit controller.stateChanged();
        auto *composer = shell->findChild<QQuickItem *>("messageComposer");
        auto *send = shell->findChild<QQuickItem *>("sendMessage");
        auto *feedback = shell->findChild<QObject *>("composerFeedback");
        QVERIFY(composer && send && feedback);
        QVERIFY(composer->isEnabled());
        composer->setProperty("text", "Draft for A");
        controller.m_state["route"] = "channel-b"; emit controller.stateChanged();
        QCOMPARE(composer->property("text").toString(), QString());
        QVERIFY(!composer->isEnabled() && !send->isEnabled());
        QVERIFY(feedback->property("text").toString().contains("Waiting for authorization"));
        controller.m_state["route"] = "channel-a"; emit controller.stateChanged();
        QCOMPARE(composer->property("text").toString(), QString("Draft for A"));
        controller.m_state["server"] = "server-b"; emit controller.stateChanged();
        QCOMPARE(composer->property("text").toString(), QString());
        composer->setProperty("text", "Draft for B");
        controller.m_state["account"] = "account-b"; emit controller.stateChanged();
        QCOMPARE(composer->property("text").toString(), QString());
        controller.m_state["account"] = "account-a"; emit controller.stateChanged();
        QCOMPARE(composer->property("text").toString(), QString("Draft for B"));
        controller.m_state["server"] = "server-a"; emit controller.stateChanged();
        QCOMPARE(composer->property("text").toString(), QString("Draft for A"));
        controller.m_state["admission"] = "pending"; emit controller.stateChanged();
        QVERIFY(!composer->isEnabled() && !send->isEnabled());
        QVERIFY(feedback->property("text").toString().contains("administrator approval"));
        controller.m_state["admission"] = "active"; controller.m_state["connected"] = false; emit controller.stateChanged();
        QVERIFY(composer->isEnabled() && !send->isEnabled());
        QVERIFY(feedback->property("text").toString().contains("Offline"));
        composer->forceActiveFocus();
        QTest::keyClick(&window, Qt::Key_Return, Qt::ShiftModifier);
        QVERIFY(composer->property("text").toString().contains('\n'));
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/messenger-composer-offline.png"));
        item->setParentItem(nullptr);
    }
    void nativeComponentThemesAndSizing() {
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/../tests/controls-gallery.qml"));
        QQuickWindow window;
        QScopedPointer<QObject> gallery(component.create());
        QVERIFY2(gallery, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(gallery.data());
        QVERIFY(item);
        window.resize(1024, 900);
        item->setParentItem(window.contentItem()); item->setSize(window.size()); window.show();
        window.raise(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
        for (const auto &theme : {QString("dark"), QString("midnight"), QString("light")}) {
            gallery->setProperty("appearance", theme);
            for (const auto scale : {1.0, 1.5, 2.0}) {
                gallery->setProperty("textScale", scale);
                QTest::qWait(100);
                auto *field = gallery->findChild<QQuickItem *>("galleryField");
                QVERIFY(field);
                field->forceActiveFocus();
                QVERIFY(field->hasActiveFocus());
                window.requestUpdate();
                const auto frame = window.grabWindow();
                QVERIFY(!frame.isNull());
                qInfo() << "Rendered theme sizing" << theme << scale << gallery->property("effectiveBodySize") << field->property("font").value<QFont>().pixelSize() << field->height() << field->implicitHeight();
                QTRY_VERIFY_WITH_TIMEOUT(field->height() >= 14 * scale + 16, 5000);
                QVERIFY(frame.save(QStringLiteral(CORDS_TEST_OUTPUT) + "/controls-" + theme + "-" + QString::number(qRound(scale * 100)) + ".png"));
            }
        }
        auto *menu = gallery->findChild<QObject *>("galleryMenu");
        QVERIFY(menu);
        QVERIFY(QMetaObject::invokeMethod(menu, "open"));
        QTRY_VERIFY(menu->property("opened").toBool());
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/controls-menu.png"));
        QTest::keyClick(&window, Qt::Key_Escape);
        QTRY_VERIFY(!menu->property("opened").toBool());
        item->setParentItem(nullptr);
    }
    void liveAdmissionWorkflow() {
        const auto fixturePath = qEnvironmentVariable("CORDS_QT_ADMISSION_FIXTURE");
        if (fixturePath.isEmpty()) QSKIP("Explicit disposable native admission fixture not selected");
        QFile file(fixturePath + "/ready.json");
        QVERIFY(file.open(QIODevice::ReadOnly));
        const auto ready = QJsonDocument::fromJson(file.readAll()).object();
        QCOMPARE(ready["fixture"].toString(), QString("native-admission-v1"));
        const auto migrations = QStringLiteral(CORDS_TEST_MIGRATIONS);
        Controller owner(ready["owner_state"].toString(), migrations, ready["ca"].toString());
        Controller applicant(ready["applicant_state"].toString(), migrations, ready["ca"].toString());
        QTRY_VERIFY_WITH_TIMEOUT(!owner.state().isEmpty() && !applicant.state().isEmpty(), 10000);
        const auto password = QStringLiteral("Native admission fixture only 7319!");
        owner.unlockAccount(ready["owner_account"].toString(), password);
        applicant.unlockAccount(ready["applicant_account"].toString(), password);
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && !applicant.busy(), 30000);
        QVERIFY2(owner.state().value("error").toString().isEmpty(), qPrintable(owner.state().value("error").toString()));
        // Exercise the actual Add Server command, rather than a pre-seeded
        // pending account or a separate Authenticate action.
        QCOMPARE(applicant.state().value("server").toString(), QString());
        owner.loadRequests();
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && owner.state().value("requestsLoaded").toBool(), 30000);
        QCOMPARE(owner.requests()->rowCount(), 0);
        applicant.trustServer(ready["origin"].toString());
        QTRY_COMPARE_WITH_TIMEOUT(applicant.state().value("admission").toString(), QString("pending"), 30000);
        QTRY_VERIFY_WITH_TIMEOUT(!applicant.busy(), 30000);
        QVERIFY2(applicant.state().value("error").toString().isEmpty(), qPrintable(applicant.state().value("error").toString()));
        // Repeating the add/status action must not duplicate the request.
        applicant.trustServer(ready["origin"].toString());
        QTRY_VERIFY_WITH_TIMEOUT(!applicant.busy(), 30000);
        QCOMPARE(applicant.state().value("admission").toString(), QString("pending"));
        QCOMPARE(applicant.channels()->rowCount(), 0);
        QCOMPARE(applicant.members()->rowCount(), 0);
        QVERIFY(!applicant.state().value("canApprove").toBool());
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/Shell.qml"));
        QQuickWindow ownerWindow, applicantWindow;
        QScopedPointer<QObject> ownerShell(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&owner)}}));
        QScopedPointer<QObject> applicantShell(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&applicant)}}));
        QVERIFY2(ownerShell && applicantShell, qPrintable(component.errorString()));
        auto *ownerItem = qobject_cast<QQuickItem *>(ownerShell.data());
        auto *applicantItem = qobject_cast<QQuickItem *>(applicantShell.data());
        QVERIFY(ownerItem && applicantItem);
        ownerWindow.resize(1024, 768); applicantWindow.resize(1024, 768);
        ownerItem->setParentItem(ownerWindow.contentItem()); ownerItem->setSize(ownerWindow.size());
        applicantItem->setParentItem(applicantWindow.contentItem()); applicantItem->setSize(applicantWindow.size());
        ownerWindow.show(); applicantWindow.show();
        emit owner.stateChanged(); emit applicant.stateChanged();
        QTest::qWait(200);
        auto *empty = applicantShell->findChild<QObject *>("conversationEmptyState");
        QVERIFY(empty);
        QCOMPARE(empty->property("title").toString(), QString("Waiting for administrator approval"));
        QVERIFY(applicantWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/admission-native-pending.png"));
        ownerItem->setProperty("showAdmin", true);
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy(), 30000);
        auto *admin = ownerShell->findChild<QObject *>("serverSettings");
        QVERIFY(admin);
        QVERIFY(QMetaObject::invokeMethod(admin, "selectPage", Q_ARG(QVariant, 2)));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && owner.requests()->rowCount() == 1, 30000);
        QCOMPARE(owner.requests()->get(0).value("account").toString(), ready["applicant_account"].toString());
        const auto click = [](QQuickWindow &window, QQuickItem *button) {
            qInfo() << "Click native admission action" << button->objectName();
            window.raise();
            window.requestActivate();
            QTest::qWait(100);
            QTest::mouseClick(&window, Qt::LeftButton, Qt::NoModifier,
                button->mapToScene(QPointF(button->width() / 2, button->height() / 2)).toPoint());
            qInfo() << "Native admission action returned" << button->objectName();
        };
        const std::function<QQuickItem *(QQuickItem *, const QString &)> findVisible = [&findVisible](QQuickItem *root, const QString &name) -> QQuickItem * {
            if (root->objectName() == name && root->isVisible()) return root;
            for (auto *child : root->childItems()) if (auto *found = findVisible(child, name)) return found;
            return nullptr;
        };
        QQuickItem *reject = nullptr;
        QTRY_VERIFY((reject = findVisible(ownerItem, "rejectMembershipRequestAction")) != nullptr);
        QVERIFY(ownerWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/admission-native-review.png"));
        click(ownerWindow, reject);
        auto *confirmation = ownerShell->findChild<QObject *>("rejectMembershipRequest");
        QVERIFY(confirmation);
        QTRY_VERIFY(confirmation->property("opened").toBool());
        QVERIFY(ownerWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/admission-native-confirmation.png"));
        auto *confirm = confirmation->findChild<QQuickItem *>("confirmOperation");
        QVERIFY(confirm && confirm->isVisible());
        click(ownerWindow, confirm);
        // The authoritative queue intentionally retains rejected requests for review.
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && owner.requests()->rowCount() == 1 && owner.requests()->get(0).value("status").toString() == "rejected", 30000);
        qInfo() << "Rejection persisted; requester status" << applicant.state().value("admission");
        auto *check = empty->findChild<QQuickItem *>("emptyStateAction");
        QVERIFY(check && check->isVisible());
        click(applicantWindow, check);
        QTRY_COMPARE_WITH_TIMEOUT(applicant.state().value("admission").toString(), QString("rejected"), 30000);
        QCOMPARE(empty->property("title").toString(), QString("Your request to join was denied"));
        QVERIFY(applicantWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/admission-native-denied.png"));
        QTRY_VERIFY(!applicant.busy());
        click(applicantWindow, check);
        QTRY_COMPARE_WITH_TIMEOUT(applicant.state().value("admission").toString(), QString("pending"), 30000);
        owner.loadRequests();
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && owner.requests()->rowCount() == 1, 30000);
        QQuickItem *approve = nullptr;
        QTRY_VERIFY((approve = findVisible(ownerItem, "approveMembershipRequest")) != nullptr);
        click(ownerWindow, approve);
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && owner.requests()->rowCount() == 0, 30000);
        click(applicantWindow, check);
        QTRY_COMPARE_WITH_TIMEOUT(applicant.state().value("admission").toString(), QString("active"), 30000);
        QVERIFY(applicant.state().value("error").toString().isEmpty());
        applicant.loadDirectory("");
        QTRY_VERIFY_WITH_TIMEOUT(!applicant.busy() && applicant.members()->rowCount() == 2, 30000);
        // Server membership exposes the readable public channel automatically;
        // the owner's encrypted-only channel must not appear in navigation.
        applicant.synchronize();
        QTRY_VERIFY_WITH_TIMEOUT(!applicant.busy() && applicant.channels()->rowCount() == 1, 30000);
        QCOMPARE(applicant.channels()->get(0).value("name").toString(), QString("Public conversation"));
        const auto publicChannel = applicant.channels()->get(0).value("id").toString();
        applicant.selectChannel(publicChannel);
        QTRY_VERIFY_WITH_TIMEOUT(!applicant.busy(), 30000);
        applicant.sendMessage(publicChannel, "Native public reply without a channel join");
        QTRY_VERIFY_WITH_TIMEOUT(!applicant.busy(), 30000);
        QVERIFY2(applicant.state().value("error").toString().isEmpty(), qPrintable(applicant.state().value("error").toString()));
        QTRY_COMPARE_WITH_TIMEOUT(applicant.messages()->rowCount(), 2, 30000);
        QVERIFY(applicantWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/admission-native-approved.png"));
        applicantItem->setParentItem(nullptr); ownerItem->setParentItem(nullptr);
    }
    void administrationDraftsAndVirtualization() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"canRoles", true}, {"revision", 7}, {"server", "test-server"}, {"directoryLoaded", true}};
        QQmlEngine engine;
        const auto path = QStringLiteral(CORDS_QML_DIR);
        QQuickWindow editorWindow; editorWindow.resize(800, 780); editorWindow.show();
        QQmlComponent roleComponent(&engine, QUrl::fromLocalFile(path + "/RoleEditor.qml"));
        QScopedPointer<QObject> editor(roleComponent.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}, {"parent", QVariant::fromValue(editorWindow.contentItem())}}));
        QVERIFY2(editor, qPrintable(roleComponent.errorString()));
        const QVariantMap role{{"id", "custom-role"}, {"name", "Guides"}, {"color", "#80b5db"}, {"priority", 1}, {"permissions", QStringList{"server.read"}}, {"mentionable", false}, {"description", ""}};
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "edit", Q_ARG(QVariant, role), Q_ARG(QVariant, true)));
        QTest::qWait(100);
        QVERIFY(editorWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-polish-role-editor.png"));
        editorWindow.resize(680, 520);
        QTest::qWait(100);
        QVERIFY(editor->property("height").toReal() <= 488);
        QVERIFY(editorWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-polish-role-editor-narrow.png"));
        QCOMPARE(editor->property("reviewedRevision").toInt(), 7);
        QVERIFY(!editor->property("dirty").toBool());
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "change", Q_ARG(QVariant, "color"), Q_ARG(QVariant, "invalid")));
        QVERIFY(editor->property("dirty").toBool());
        QVERIFY(!editor->property("valid").toBool());
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "change", Q_ARG(QVariant, "color"), Q_ARG(QVariant, "#ffffff")));
        controller.m_state["revision"] = 8;
        controller.m_state["error"] = "Concurrent policy change";
        emit controller.stateChanged();
        QCOMPARE(editor->property("reviewedRevision").toInt(), 7);
        QCOMPARE(editor->property("draft").value<QJSValue>().toVariant().toMap().value("color").toString(), "#ffffff");
        auto *save = editor->findChild<QObject *>("saveRole");
        QVERIFY(save);
        QVERIFY(!save->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "close"));

        QList<QVariantMap> members;
        for (int i = 0; i < 2000; ++i) members.push_back({{"id", QString::number(i)}, {"name", QString("Person %1").arg(i)}, {"roles", QStringList{}}});
        controller.members()->replace(members);
        controller.m_busy = true;
        QQmlComponent memberComponent(&engine, QUrl::fromLocalFile(path + "/MemberWorkspace.qml"));
        QScopedPointer<QObject> workspace(memberComponent.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(workspace, qPrintable(memberComponent.errorString()));
        auto *item = qobject_cast<QQuickItem *>(workspace.data());
        QQuickWindow window; window.setColor(QColor("#252e38")); window.resize(1100, 650);
        item->setParentItem(window.contentItem()); item->setSize(window.size()); window.show();
        QTest::qWait(100);
        auto *list = workspace->findChild<QObject *>("administrativeMembers");
        QVERIFY(list);
        QCOMPARE(list->property("count").toInt(), 2000);
        const auto rendered = workspace->findChildren<QObject *>("administrativeMemberRow").size();
        QVERIFY2(rendered < 100, "Member list must not instantiate every account");
        auto *search = workspace->findChild<QObject *>("memberSearch");
        QVERIFY(search);
        search->setProperty("text", "Person 1999");
        QTRY_COMPARE(list->property("count").toInt(), 1);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-polish-members.png"));
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "selectMember", Q_ARG(QVariant, members.last())));
        QCOMPARE(workspace->property("selectedAccount").toString(), QString("1999"));
        controller.m_state["server"] = "another-server";
        emit controller.stateChanged();
        QVERIFY(workspace->property("selectedAccount").toString().isEmpty());
        QCOMPARE(search->property("text").toString(), QString());
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "selectMember", Q_ARG(QVariant, members.last())));
        controller.m_state["account"] = "another-local-account";
        emit controller.stateChanged();
        QVERIFY(workspace->property("selectedAccount").toString().isEmpty());
        item->setParentItem(nullptr);
        QQmlComponent requestsComponent(&engine, QUrl::fromLocalFile(path + "/RequestsWorkspace.qml"));
        QScopedPointer<QObject> requests(requestsComponent.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(requests, qPrintable(requestsComponent.errorString()));
        auto *requestsItem = qobject_cast<QQuickItem *>(requests.data());
        requestsItem->setParentItem(window.contentItem());
        requestsItem->setSize(window.size());
        auto *rejection = requests->findChild<QObject *>("rejectMembershipRequest");
        QVERIFY(rejection);
        QVERIFY(QMetaObject::invokeMethod(rejection, "open"));
        QTRY_VERIFY(requests->property("modalOpen").toBool());
        controller.m_state["account"] = "third-local-account";
        emit controller.stateChanged();
        QTRY_VERIFY(!requests->property("modalOpen").toBool());
        requestsItem->setParentItem(nullptr);
    }
    void administrationDraftsFollowReviewedContext() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"account", "first-account"}, {"server", "first-server"}};
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AdminPane.qml"));
        QScopedPointer<QObject> admin(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(admin, qPrintable(component.errorString()));
        QQuickWindow window; window.resize(1180, 760); window.show();
        auto *item = qobject_cast<QQuickItem *>(admin.data());
        item->setParentItem(window.contentItem()); item->setSize(window.size());
        auto *confirmation = admin->findChild<QObject *>("serverAdministrationConfirmation");
        QVERIFY(confirmation);
        const QStringList fields{"ownerCode", "successorAccount", "designation", "targetDevice", "replacementName"};
        for (const auto &scopeKey : {QString("server"), QString("account")}) {
            for (const auto &field : fields) {
                auto *input = admin->findChild<QObject *>(field);
                QVERIFY(input);
                input->setProperty("text", "private-reviewed-value");
            }
            QVERIFY(QMetaObject::invokeMethod(confirmation, "open"));
            QTRY_VERIFY(admin->property("modalOpen").toBool());
            controller.m_state[scopeKey] = "changed-" + scopeKey;
            emit controller.stateChanged();
            QTRY_VERIFY(!admin->property("modalOpen").toBool());
            for (const auto &field : fields)
                QCOMPARE(admin->findChild<QObject *>(field)->property("text").toString(), QString());
        }
        auto *target = admin->findChild<QObject *>("targetDevice");
        target->setProperty("text", "previous-channel-device");
        QVERIFY(QMetaObject::invokeMethod(confirmation, "open"));
        admin->setProperty("route", "different-channel");
        QTRY_VERIFY(!admin->property("modalOpen").toBool());
        QCOMPARE(target->property("text").toString(), QString());
        item->setParentItem(nullptr);
    }
    void serverProfileDraftSurvivesConcurrentRefresh() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        QVariantMap profile{{"supported", true}, {"loaded", true}, {"canEdit", true}, {"revision", 3}, {"name", "Community"}, {"description", "Original"}};
        controller.m_state = {{"server", "test-server"}, {"admission", "active"}, {"serverProfile", profile}};
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/ServerProfileForm.qml"));
        QScopedPointer<QObject> form(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(form, qPrintable(component.errorString()));
        auto *name = form->findChild<QObject *>("serverProfileName");
        auto *background = form->findChild<QObject *>("serverProfileBackground");
        auto *save = form->findChild<QQuickItem *>("saveServerProfile");
        QVERIFY(name && save && background);
        background->setProperty("text", "#12345g");
        QVERIFY(form->property("dirty").toBool());
        QVERIFY(!save->isEnabled());
        background->setProperty("text", "#205566");
        QVERIFY(save->isEnabled());
        name->setProperty("text", "Draft name");
        QVERIFY(form->property("dirty").toBool());
        QVERIFY(save->isEnabled());
        profile["revision"] = 4;
        profile["name"] = "Remote name";
        controller.m_state["serverProfile"] = profile;
        emit controller.stateChanged();
        QCOMPARE(name->property("text").toString(), QString("Draft name"));
        QCOMPARE(background->property("text").toString(), QString("#205566"));
        QVERIFY(!save->isEnabled());
        QVERIFY(QMetaObject::invokeMethod(form.data(), "reset"));
        QCOMPARE(name->property("text").toString(), QString("Remote name"));
        QVERIFY(!form->property("dirty").toBool());
        QCOMPARE(background->property("text").toString(), QString("#314452"));
        name->setProperty("text", "Private unsaved draft");
        controller.m_state["account"] = "different-local-account";
        emit controller.stateChanged();
        QCOMPARE(name->property("text").toString(), QString("Remote name"));
        QVERIFY(!form->property("dirty").toBool());
    }
    void nativeAvatarCropPreview() {
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AvatarEditor.qml"));
        QScopedPointer<QObject> editor(component.create());
        QVERIFY2(editor, qPrintable(component.errorString()));
        QQuickWindow window; window.resize(680, 640); window.show();
        QVERIFY(editor->setProperty("parent", QVariant::fromValue(window.contentItem())));
        QFile fixture(QStringLiteral(CORDS_TEST_IMAGE));
        QVERIFY(fixture.open(QIODevice::ReadOnly));
        const QString image = "data:image/jxl;base64," + QString::fromLatin1(fixture.readAll().toBase64());
        editor->setProperty("draft", QVariantMap{{"id", "local-crop-fixture"}, {"image", image}, {"width", 100}, {"height", 100}});
        QTRY_VERIFY(editor->property("visible").toBool());
        auto *zoom = editor->findChild<QObject *>("avatarCropZoom");
        auto *preview = editor->findChild<QObject *>("avatarCropPreview");
        QVERIFY(zoom && preview);
        zoom->setProperty("value", 2.0);
        QCOMPARE(preview->property("zoom").toDouble(), 2.0);
        editor->setProperty("centerX", 0.0);
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "constrain"));
        QCOMPARE(editor->property("centerX").toDouble(), 0.25);
        auto *decoded = preview->findChild<JxlImage *>("avatarPlayback");
        QVERIFY(decoded);
        QTRY_VERIFY_WITH_TIMEOUT(decoded->ready(), 90000);
        QVERIFY(decoded->error().isEmpty());
        QTest::qWait(100);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/ui-polish-avatar-crop.png"));
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
        auto *overridePolicy = picker->findChild<QObject *>("ignoreWeakPasswords");
        QVERIFY(overridePolicy);
        QCOMPARE(overridePolicy->property("text").toString(), QString("Ignore weak passwords"));
        for (const QString &weak : {QString(), QString("x"), QString("password")}) {
            password->setProperty("text", weak);
            confirmation->setProperty("text", weak);
            overridePolicy->setProperty("checked", false);
            QVERIFY(!submit->isEnabled());
            overridePolicy->setProperty("checked", true);
            QVERIFY(submit->isEnabled());
            confirmation->setProperty("text", weak + "mismatch");
            QVERIFY(!submit->isEnabled());
        }
        overridePolicy->setProperty("checked", false);
        password->setProperty("text", "Mango Lantern selector acceptance 83");
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
