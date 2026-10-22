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
    void liveRoleManagementWorkflow() {
        const auto fixturePath = qEnvironmentVariable("CORDS_NATIVE_ROLE_FIXTURE");
        if (fixturePath.isEmpty()) QSKIP("Explicit disposable native role fixture not selected");
        QFile file(fixturePath + "/ready.json");
        QVERIFY(file.open(QIODevice::ReadOnly));
        const auto ready = QJsonDocument::fromJson(file.readAll()).object();
        QCOMPARE(ready["fixture"].toString(), QString("native-roles-v1"));
        Controller owner(ready["owner_state"].toString(), QStringLiteral(CORDS_TEST_MIGRATIONS), ready["ca"].toString());
        Controller member(ready["member_state"].toString(), QStringLiteral(CORDS_TEST_MIGRATIONS), ready["ca"].toString());
        QTRY_VERIFY_WITH_TIMEOUT(!owner.state().isEmpty() && !member.state().isEmpty(), 10000);
        const auto password = QStringLiteral("Native role fixture only 9841!");
        owner.unlockAccount(ready["owner_account"].toString(), password);
        member.unlockAccount(ready["member_account"].toString(), password);
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && !member.busy(), 30000);
        QVERIFY2(owner.state().value("error").toString().isEmpty(), qPrintable(owner.state().value("error").toString()));
        QVERIFY2(member.state().value("error").toString().isEmpty(), qPrintable(member.state().value("error").toString()));
        owner.loadRoles();
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy(), 30000);
        QVERIFY(owner.state().value("rolesActive").toBool());
        QQmlEngine engine;
        QQuickWindow ownerWindow, memberWindow;
        ownerWindow.setColor(QColor("#25262b")); ownerWindow.resize(700, 700); ownerWindow.show();
        memberWindow.setColor(QColor("#25262b")); memberWindow.resize(500, 700); memberWindow.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/RoleMembersWorkspace.qml"));
        QScopedPointer<QObject> workspace(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&owner)},
            {"parent", QVariant::fromValue(ownerWindow.contentItem())}, {"width", 680}, {"roleId", ready["role"].toString()}, {"saved", true}}));
        QVERIFY2(workspace, qPrintable(component.errorString()));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && workspace->property("current").toBool(), 30000);
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "setAdding", Q_ARG(QVariant, true)));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && workspace->property("current").toBool(), 30000);
        auto *search = workspace->findChild<QObject *>("roleMemberSearch"); QVERIFY(search);
        search->setProperty("text", ready["member_account"].toString());
        auto *list = workspace->findChild<QObject *>("roleMemberList"); QVERIFY(list);
        QTRY_COMPARE(list->property("count").toInt(), 1);
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "selectAccount", Q_ARG(QVariant, ready["member_account"].toString()), Q_ARG(QVariant, true)));
        search->setProperty("text", ready["other_account"].toString());
        QTRY_COMPARE(list->property("count").toInt(), 1);
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "selectAccount", Q_ARG(QVariant, ready["other_account"].toString()), Q_ARG(QVariant, true)));
        auto *add = workspace->findChild<QObject *>("assignSelectedRoleMembers"); QVERIFY(add);
        QVERIFY(add->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(add, "clicked"));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && workspace->property("confirmed").toInt() == 2 && workspace->property("current").toBool(), 30000);
        QVERIFY2(owner.state().value("error").toString().isEmpty(), qPrintable(owner.state().value("error").toString()));
        QCOMPARE(owner.state().value("roleMembers").toMap().value("memberCount").toInt(), 2);
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "setAdding", Q_ARG(QVariant, false)));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && workspace->property("current").toBool(), 30000);
        QTest::qWait(150);
        QVERIFY(ownerWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/role-members-live-assigned.png"));
        QQmlComponent cardComponent(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AccountCard.qml"));
        QScopedPointer<QObject> readOnly(cardComponent.createWithInitialProperties({{"desktop", QVariant::fromValue(&member)},
            {"parent", QVariant::fromValue(memberWindow.contentItem())}, {"account", ready["other_account"].toString()}}));
        QVERIFY2(readOnly, qPrintable(cardComponent.errorString()));
        QVERIFY(QMetaObject::invokeMethod(readOnly.data(), "open"));
        QTRY_VERIFY_WITH_TIMEOUT(!member.busy() && readOnly->property("roleActionsLoaded").toBool(), 30000);
        QVERIFY(member.state().value("roleTarget").toMap().value("assignable").toStringList().isEmpty());
        QVERIFY(member.state().value("roleTarget").toMap().value("removable").toStringList().isEmpty());
        QCOMPARE(readOnly->property("visibleRoles").value<QJSValue>().toVariant().toList().size(), 1);
        auto *readOnlyAdd = readOnly->findChild<QObject *>("profileAddRole"); QVERIFY(readOnlyAdd);
        QVERIFY(!readOnlyAdd->property("visible").toBool());
        QVERIFY(QMetaObject::invokeMethod(readOnly.data(), "close"));
        // Exercise the same removal handler used by Manage Members confirmation.
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "start", Q_ARG(QVariant, QStringList{ready["other_account"].toString()}), Q_ARG(QVariant, true)));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && workspace->property("confirmed").toInt() == 1 && workspace->property("current").toBool(), 30000);
        QCOMPARE(owner.state().value("roleMembers").toMap().value("memberCount").toInt(), 1);
        QScopedPointer<QObject> card(cardComponent.createWithInitialProperties({{"desktop", QVariant::fromValue(&owner)},
            {"parent", QVariant::fromValue(ownerWindow.contentItem())}, {"account", ready["member_account"].toString()}}));
        QVERIFY2(card, qPrintable(cardComponent.errorString()));
        QVERIFY(QMetaObject::invokeMethod(card.data(), "open"));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && card->property("roleActionsLoaded").toBool(), 30000);
        QVERIFY(owner.state().value("roleTarget").toMap().value("removable").toStringList().contains(ready["role"].toString()));
        QVERIFY(QMetaObject::invokeMethod(card.data(), "changeRole", Q_ARG(QVariant, ready["role"].toString()), Q_ARG(QVariant, true)));
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && card->property("roleActionsLoaded").toBool() && card->property("visibleRoles").value<QJSValue>().toVariant().toList().isEmpty(), 30000);
        QVERIFY2(owner.state().value("error").toString().isEmpty(), qPrintable(owner.state().value("error").toString()));
        QVERIFY(card->property("opened").toBool());
        QTRY_VERIFY_WITH_TIMEOUT(workspace->property("current").toBool(), 30000);
        QCOMPARE(owner.state().value("roleMembers").toMap().value("memberCount").toInt(), 0);
        owner.lockAccount(); member.lockAccount();
        QTRY_VERIFY_WITH_TIMEOUT(!owner.busy() && !member.busy(), 30000);
        QVERIFY(!owner.state().value("unlocked").toBool() && !member.state().value("unlocked").toBool());
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
        QQuickItem *requestAvatar = nullptr;
        QTRY_VERIFY((requestAvatar = findVisible(ownerItem, "membershipRequestAvatar")) != nullptr);
        QFile avatarFile(fixturePath + "/applicant-avatar.jxl");
        QVERIFY(avatarFile.open(QIODevice::ReadOnly));
        const auto expectedAvatar = "data:image/jxl;base64," + avatarFile.readAll().toBase64();
        QTRY_COMPARE_WITH_TIMEOUT(requestAvatar->property("source").toUrl().toString(), QString::fromLatin1(expectedAvatar), 30000);
        QTRY_VERIFY_WITH_TIMEOUT(requestAvatar->property("ready").toBool(), 30000);
        QCOMPARE(requestAvatar->property("shape").toString(), QString("square"));
        ownerWindow.raise(); ownerWindow.requestActivate();
        QTest::mouseMove(&ownerWindow, requestAvatar->mapToScene(QPointF(50, 10)).toPoint());
        QTRY_VERIFY(requestAvatar->property("playing").toBool());
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
        // Existing public history must arrive before this new member sends
        // anything; first-send initialization must not conceal a loading bug.
        QTRY_COMPARE_WITH_TIMEOUT(applicant.messages()->rowCount(), 1, 30000);
        QVERIFY(applicant.state().value("error").toString().isEmpty());
        QVERIFY(applicantWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/public-history-before-first-send.png"));
        applicant.sendMessage(publicChannel, "Native public reply without a channel join");
        QTRY_VERIFY_WITH_TIMEOUT(!applicant.busy(), 30000);
        QVERIFY2(applicant.state().value("error").toString().isEmpty(), qPrintable(applicant.state().value("error").toString()));
        QTRY_COMPARE_WITH_TIMEOUT(applicant.messages()->rowCount(), 2, 30000);
        QVERIFY(applicantWindow.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/admission-native-approved.png"));
        applicantItem->setParentItem(nullptr); ownerItem->setParentItem(nullptr);
    }
    void conversationEmptyHistoryStates() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"account", "test-account"}, {"server", "test-server"},
            {"route", "empty-channel"}, {"admission", "active"}, {"connected", true},
            {"historyLoaded", false}};
        controller.channels()->replace({{{"id", "empty-channel"}, {"name", "empty"}, {"mode", "public"}, {"joined", true}}});
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/Shell.qml"));
        QScopedPointer<QObject> shell(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(shell, qPrintable(component.errorString()));
        auto empty = shell->findChild<QObject *>("conversationEmptyState");
        QVERIFY(empty);
        QCOMPARE(empty->property("title").toString(), QString("Messages haven't been loaded on this device"));
        QCOMPARE(empty->property("actionText").toString(), QString("Load messages"));
        controller.m_busy = true;
        emit controller.busyChanged();
        QCOMPARE(empty->property("title").toString(), QString("Loading messages…"));
        controller.m_busy = false;
        emit controller.busyChanged();
        controller.m_state["historyLoaded"] = true;
        emit controller.stateChanged();
        QCOMPARE(empty->property("title").toString(), QString("No messages yet"));
        QVERIFY(empty->property("actionText").toString().isEmpty());
        controller.channels()->replace({{{"id", "empty-channel"}, {"name", "empty"}, {"mode", "encrypted"}, {"joined", true}}});
        emit controller.stateChanged();
        QCOMPARE(empty->property("title").toString(), QString("No messages available yet"));
        QVERIFY(empty->property("explanation").toString().contains("before this device was added"));
        controller.m_state["error"] = "Synchronization failed";
        emit controller.stateChanged();
        QVERIFY(!empty->property("historyLoaded").toBool());
        controller.m_state["error"] = "";
        controller.m_state["connected"] = false;
        emit controller.stateChanged();
        QVERIFY(!empty->property("historyLoaded").toBool());
        controller.m_state["connected"] = true;
        controller.m_state["route"] = "unloaded-channel";
        controller.m_state["historyLoaded"] = false;
        emit controller.stateChanged();
        QCOMPARE(empty->property("title").toString(), QString("Messages haven't been loaded on this device"));
    }
    void administrationDraftsAndVirtualization() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"canCreateRoles", true}, {"canRoles", true}, {"revision", 7}, {"server", "test-server"}, {"directoryLoaded", true}};
        QQmlEngine engine;
        const auto path = QStringLiteral(CORDS_QML_DIR);
        QQuickWindow editorWindow; editorWindow.resize(800, 780); editorWindow.show();
        QQmlComponent roleComponent(&engine, QUrl::fromLocalFile(path + "/RoleEditor.qml"));
        QScopedPointer<QObject> editor(roleComponent.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}, {"parent", QVariant::fromValue(editorWindow.contentItem())}}));
        QVERIFY2(editor, qPrintable(roleComponent.errorString()));
        controller.roles()->replace({{{"id", "custom-role"}, {"editable", true}, {"deletable", true}}});
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
    void inlineRoleEditorPreservesDraftAndSearches() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"canCreateRoles", true}, {"canRoles", true}, {"revision", 7}, {"account", "account"}, {"server", "server"}};
        controller.roles()->replace({{{"editable", true}, {"deletable", true}, {"memberCount", 12}, {"id", "first"}, {"name", "Guides"}, {"color", "#80b5db"}, {"priority", 2}}, {{"editable", true}, {"deletable", true}, {"id", "second"}, {"name", "Helpers"}, {"color", "#78c99a"}, {"priority", 1}}});
        QQmlEngine engine;
        QQuickWindow window; window.setColor(QColor("#32343b")); window.resize(1180, 760); window.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AdminPane.qml"));
        QScopedPointer<QObject> admin(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(admin, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(admin.data());
        item->setParentItem(window.contentItem()); item->setSize(window.size());
        admin->setProperty("page", 3);
        auto *editor = admin->findChild<QObject *>("roleEditor");
        QVERIFY(editor);
        const QVariantMap first{{"editable", true}, {"deletable", true}, {"memberCount", 12}, {"id", "first"}, {"name", "Guides"}, {"color", "#80b5db"}, {"permissions", QStringList{}}};
        const QVariantMap second{{"editable", true}, {"deletable", true}, {"id", "second"}, {"name", "Helpers"}, {"color", "#78c99a"}, {"permissions", QStringList{}}};
        QVERIFY(QMetaObject::invokeMethod(editor, "edit", Q_ARG(QVariant, first), Q_ARG(QVariant, true)));
        QVERIFY(QMetaObject::invokeMethod(editor, "change", Q_ARG(QVariant, "name"), Q_ARG(QVariant, "Unsaved guides")));
        QVERIFY(QMetaObject::invokeMethod(editor, "edit", Q_ARG(QVariant, second), Q_ARG(QVariant, true)));
        auto draft = [editor] { return editor->property("draft").value<QJSValue>().toVariant().toMap(); };
        QCOMPARE(draft().value("id").toString(), "first");
        auto *discard = editor->findChild<QObject *>("discardRoleConfirmation");
        QTRY_VERIFY(discard->property("visible").toBool());
        QVERIFY(QMetaObject::invokeMethod(discard, "close"));
        QCOMPARE(draft().value("name").toString(), "Unsaved guides");
        QVERIFY(QMetaObject::invokeMethod(editor, "edit", Q_ARG(QVariant, second), Q_ARG(QVariant, true)));
        QVERIFY(QMetaObject::invokeMethod(discard->findChild<QObject *>("confirmOperation"), "clicked"));
        QCOMPARE(draft().value("id").toString(), "second");
        QVERIFY(!editor->property("dirty").toBool());
        editor->setProperty("tab", 1);
        auto *search = editor->findChild<QObject *>("permissionSearch");
        auto *permissions = editor->findChild<QObject *>("rolePermissions");
        QVERIFY(search); QVERIFY(permissions);
        search->setProperty("text", "MLS");
        QTRY_COMPARE(permissions->property("matching").value<QJSValue>().toVariant().toList().size(), 3);
        search->setProperty("text", "no-such-capability");
        QTRY_VERIFY(permissions->property("matching").value<QJSValue>().toVariant().toList().isEmpty());
        search->setProperty("text", "");
        editor->setProperty("tab", 0);
        QTest::qWait(100);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/roles-inline-wide.png"));
        window.resize(760, 620); item->setSize(window.size());
        QTest::qWait(100);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/roles-inline-narrow.png"));
        auto *editorItem = qobject_cast<QQuickItem *>(editor);
        QVERIFY(editorItem->x() + editorItem->width() <= item->width());
        QVERIFY(editorItem->width() >= 180);
        QVERIFY(QMetaObject::invokeMethod(editor, "change", Q_ARG(QVariant, "name"), Q_ARG(QVariant, "Keep this draft")));
        QVERIFY(QMetaObject::invokeMethod(admin.data(), "selectPage", Q_ARG(QVariant, 1)));
        QCOMPARE(admin->property("page").toInt(), 3);
        auto *leave = admin->findChild<QObject *>("leaveRolesConfirmation");
        QTRY_VERIFY(leave->property("visible").toBool());
        QVERIFY(QMetaObject::invokeMethod(leave, "close"));
        QCOMPARE(draft().value("name").toString(), "Keep this draft");
        QVERIFY(QMetaObject::invokeMethod(editor, "close"));
        QVERIFY(QMetaObject::invokeMethod(editor, "edit", Q_ARG(QVariant, second), Q_ARG(QVariant, true)));
        QTest::keyClick(&window, Qt::Key_Escape);
        QTRY_VERIFY(!editor->property("visible").toBool());
        QCOMPARE(admin->property("page").toInt(), 3);
    }
    void roleMemberPagesRequireFreshHintsAndPreserveFailedReceipts() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_busy = true;
        controller.m_state = {{"account", "actor"}, {"server", "server"}, {"revision", 7}, {"rolesActive", true}};
        QQmlEngine engine;
        QQuickWindow window; window.setColor(QColor("#25262b")); window.resize(480, 680); window.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/RoleMembersWorkspace.qml"));
        QScopedPointer<QObject> workspace(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)},
            {"parent", QVariant::fromValue(window.contentItem())}, {"width", 460}, {"roleId", "role"}, {"saved", true}}));
        QVERIFY2(workspace, qPrintable(component.errorString()));
        workspace->setProperty("adding", true);
        workspace->setProperty("pendingRead", false);
        const auto page = [](int revision) -> QVariantMap {
            return {{"role", "role"}, {"assigned", false}, {"revision", revision}, {"loaded", true}, {"nextAfter", "cursor"}, {"memberCount", 21},
                {"rows", QVariantList{QVariantMap{{"account", "eligible"}, {"canAssign", true}, {"canRemove", false}},
                    QVariantMap{{"account", "protected"}, {"canAssign", false}, {"canRemove", false}}}}};
        };
        controller.m_state["roleMembers"] = page(7);
        controller.m_state["people"] = QVariantMap{{"eligible", QVariantMap{{"name", "An exceptionally long signed profile name for a narrow member list"}}}};
        emit controller.stateChanged();
        controller.m_busy = false;
        emit controller.busyChanged();
        QVERIFY(workspace->property("current").toBool());
        QVERIFY(workspace->property("actionable").toBool());
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "selectAccount", Q_ARG(QVariant, "protected"), Q_ARG(QVariant, true)));
        QCOMPARE(workspace->property("selected").value<QJSValue>().toVariant().toList().size(), 0);
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "selectAccount", Q_ARG(QVariant, "eligible"), Q_ARG(QVariant, true)));
        QCOMPARE(workspace->property("selected").value<QJSValue>().toVariant().toList().size(), 1);
        auto *search = workspace->findChild<QObject *>("roleMemberSearch"); QVERIFY(search);
        search->setProperty("text", "signed profile");
        auto *list = workspace->findChild<QObject *>("roleMemberList"); QVERIFY(list);
        QTRY_COMPARE(list->property("count").toInt(), 1);
        QTest::qWait(120);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/role-members-narrow.png"));
        search->setProperty("text", "unknown");
        QTRY_COMPARE(list->property("count").toInt(), 0);
        search->setProperty("text", "");
        controller.m_state["revision"] = 8; emit controller.stateChanged();
        QVERIFY(!workspace->property("current").toBool());
        QVERIFY(!workspace->property("actionable").toBool());
        controller.m_state["revision"] = 7; emit controller.stateChanged();
        workspace->setProperty("blocked", true);
        QVERIFY(!workspace->property("actionable").toBool());
        workspace->setProperty("blocked", false);
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "start", Q_ARG(QVariant, QStringList{"eligible"}), Q_ARG(QVariant, false)));
        QVERIFY(controller.busy());
        const auto batch = workspace->property("batch").value<QJSValue>().toVariant().toList();
        QCOMPARE(batch.size(), 1);
        const auto receipt = batch.first().toMap().value("requestId").toString();
        QVERIFY(!receipt.isEmpty());
        // Completion injection tests QML failure/retry state only. The real TLS
        // worker acceptance independently exercises server mutation receipts.
        controller.m_state["error"] = "Connection interrupted";
        controller.m_busy = false; emit controller.stateChanged();
        QVERIFY(workspace->property("failed").toBool());
        QCOMPARE(workspace->property("confirmed").toInt(), 0);
        QVERIFY(workspace->property("message").toString().contains("Confirmed 0 of 1"));
        workspace->setProperty("stage", "");
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "writeNext"));
        QCOMPARE(workspace->property("batch").value<QJSValue>().toVariant().toList().first().toMap().value("requestId").toString(), receipt);
        controller.m_state["server"] = "other"; emit controller.stateChanged();
        QVERIFY(!workspace->property("current").toBool());
        QVERIFY(!workspace->property("failed").toBool());
        QCOMPARE(workspace->property("selected").value<QJSValue>().toVariant().toList().size(), 0);
    }
    void roleEditorRequiresCurrentTargetMetadata() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"canRoles", true}, {"account", "actor"}, {"server", "server"}, {"revision", 1}};
        QQmlEngine engine;
        QQuickWindow window; window.resize(800, 780); window.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/RoleEditor.qml"));
        QScopedPointer<QObject> editor(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}, {"parent", QVariant::fromValue(window.contentItem())}}));
        QVERIFY2(editor, qPrintable(component.errorString()));
        const QVariantMap draft{{"id", "protected"}, {"name", "Role"}, {"color", "#123456"}, {"editable", true}, {"deletable", true}, {"permissions", QStringList{"server.invite.create"}}, {"mentionable", true}};
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "edit", Q_ARG(QVariant, draft), Q_ARG(QVariant, true)));
        QVERIFY(!editor->property("canEdit").toBool());
        controller.roles()->replace({{{"id", "protected"}, {"editable", true}, {"deletable", true}}});
        QTRY_VERIFY(editor->property("canEdit").toBool());
        QVERIFY(editor->property("canDelete").toBool());
        auto *mentionable = editor->findChild<QObject *>("roleMentionable"); QVERIFY(mentionable);
        QVERIFY(!mentionable->property("enabled").toBool());
        QVERIFY(mentionable->property("checked").toBool());
        editor->setProperty("tab", 1);
        const std::function<QQuickItem *(QQuickItem *)> findInvite = [&findInvite](QQuickItem *root) -> QQuickItem * {
            if (root->objectName() == "rolePermission-server.invite.create") return root;
            for (auto *child : root->childItems()) if (auto *found = findInvite(child)) return found;
            return nullptr;
        };
        auto *invites = findInvite(qobject_cast<QQuickItem *>(editor.data()));
        QVERIFY(invites);
        QVERIFY(!invites->property("enabled").toBool());
        QVERIFY(invites->property("checked").toBool());
        controller.roles()->replace({{{"id", "protected"}, {"editable", false}, {"deletable", false}}});
        QTRY_VERIFY(!editor->property("canEdit").toBool());
        QVERIFY(!editor->property("canDelete").toBool());
    }
    void roleReorderKeepsProtectedAnchorsAndSupportsKeyboardAndDrag() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"account", "actor"}, {"server", "server"}, {"revision", 3}};
        controller.roles()->replace({
            {{"id", "builtin:admin"}, {"name", "admin"}, {"priority", 200}, {"editable", false}},
            {{"id", "builtin:moderator"}, {"name", "moderator"}, {"priority", 100}, {"editable", false}},
            {{"id", "a"}, {"name", "Alpha"}, {"priority", 10}, {"editable", true}},
            {{"id", "b"}, {"name", "Beta"}, {"priority", 10}, {"editable", true}},
            {{"id", "builtin:default"}, {"name", "default"}, {"priority", 0}, {"editable", true}}});
        QQmlEngine engine;
        QQuickWindow window; window.resize(900, 700); window.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/RolesWorkspace.qml"));
        QScopedPointer<QObject> workspace(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(workspace, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(workspace.data());
        item->setParentItem(window.contentItem()); item->setSize(window.size());
        QVariant result;
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "reorderProposal", Q_RETURN_ARG(QVariant, result), Q_ARG(QVariant, "b"), Q_ARG(QVariant, "a"), Q_ARG(QVariant, true)));
        const auto proposal = result.value<QJSValue>().toVariant().toList();
        QCOMPARE(proposal.size(), 5);
        QCOMPARE(proposal[0].toMap().value("priority").toInt(), 200);
        QCOMPARE(proposal[1].toMap().value("priority").toInt(), 100);
        QCOMPARE(proposal[2].toMap().value("id").toString(), "b");
        QVERIFY(proposal[2].toMap().value("priority").toInt() > proposal[3].toMap().value("priority").toInt());
        QCOMPARE(proposal[4].toMap().value("priority").toInt(), 0);
        const std::function<QQuickItem *(QQuickItem *, const QString &)> findItem = [&findItem](QQuickItem *root, const QString &name) -> QQuickItem * {
            if (root->objectName() == name) return root;
            for (auto *child : root->childItems()) if (auto *found = findItem(child, name)) return found;
            return nullptr;
        };
        QQuickItem *alpha = nullptr, *beta = nullptr;
        QTRY_VERIFY((alpha = findItem(item, "roleRow-a")));
        QTRY_VERIFY((beta = findItem(item, "roleRow-b")));
        auto *confirm = workspace->findChild<QObject *>("reorderRolesConfirmation");
        QVERIFY(confirm);
        alpha->forceActiveFocus();
        QTest::keyClick(&window, Qt::Key_Down, Qt::AltModifier);
        QTRY_VERIFY(confirm->property("visible").toBool());
        const auto requestId = workspace->property("reorderRequestId").toString();
        QVERIFY(!requestId.isEmpty());
        QVERIFY(QMetaObject::invokeMethod(confirm, "close"));
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "moveAdjacent", Q_ARG(QVariant, "a"), Q_ARG(QVariant, 1)));
        QCOMPARE(workspace->property("reorderRequestId").toString(), requestId);
        QVERIFY(QMetaObject::invokeMethod(confirm, "close"));
        auto *handle = findItem(alpha, "roleDragHandle");
        QVERIFY(handle);
        const QPoint start = handle->mapToScene(QPointF(handle->width()/2, handle->height()/2)).toPoint();
        const QPoint finish = beta->mapToScene(QPointF(beta->width()/2, beta->height()-5)).toPoint();
        QTest::mousePress(&window, Qt::LeftButton, Qt::NoModifier, start);
        QTest::mouseMove(&window, start + QPoint(0, 15), 30);
        QTest::mouseMove(&window, finish, 30);
        QTest::mouseRelease(&window, Qt::LeftButton, Qt::NoModifier, finish, 30);
        QTRY_VERIFY(confirm->property("visible").toBool());
        controller.m_state["server"] = "other"; emit controller.stateChanged();
        QTRY_VERIFY(!confirm->property("visible").toBool());
        QVERIFY(workspace->property("reorderPayload").toString().isEmpty());
        QVERIFY(QMetaObject::invokeMethod(workspace.data(), "moveRole", Q_ARG(QVariant, "a"), Q_ARG(QVariant, "builtin:admin"), Q_ARG(QVariant, true)));
        QVERIFY(!workspace->property("reorderError").toString().isEmpty());
        QVERIFY(!confirm->property("visible").toBool());
    }
    void recoveryPresentationUsesScopedCandidatesAndExplicitConfirmation() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_busy = true; // Presentation fixture only; no service authority claim.
        controller.m_state = {{"account", "owner"}, {"server", "server"}, {"canRecover", true}, {"recoveriesLoaded", true}};
        controller.recoveries()->replace({{{"id", "blocked-route"}, {"epoch", 8}, {"revision", 4}, {"committerAvailable", false}}});
        QQmlEngine engine;
        QQuickWindow window; window.resize(1000, 800); window.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AdminPane.qml"));
        QScopedPointer<QObject> admin(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}}));
        QVERIFY2(admin, qPrintable(component.errorString()));
        auto *item = qobject_cast<QQuickItem *>(admin.data());
        item->setParentItem(window.contentItem()); item->setSize(window.size());
        admin->setProperty("page", 4);
        const std::function<QQuickItem *(QQuickItem *, const QString &)> findItem = [&findItem](QQuickItem *root, const QString &name) -> QQuickItem * {
            if (root->objectName() == name) return root;
            for (auto *child : root->childItems()) if (auto *found = findItem(child, name)) return found;
            return nullptr;
        };
        QQuickItem *name = nullptr, *recover = nullptr;
        QTRY_VERIFY((name = findItem(item, "channelRecoveryName")));
        QTRY_VERIFY((recover = findItem(item, "recoverEncryptedChannel")));
        QVERIFY(!recover->property("enabled").toBool());
        controller.m_busy = false; emit controller.busyChanged();
        QVERIFY(!recover->property("enabled").toBool());
        name->setProperty("text", "Fresh encrypted group");
        QTRY_VERIFY(recover->property("enabled").toBool());
        QVERIFY(QMetaObject::invokeMethod(recover, "clicked"));
        auto *confirm = admin->findChild<QObject *>("serverAdministrationConfirmation");
        QVERIFY(confirm);
        QTRY_VERIFY(confirm->property("visible").toBool());
        QVERIFY(confirm->property("explanation").toString().contains("No old keys or history transfer"));
        controller.m_state.insert("server", "another-server"); emit controller.stateChanged();
        QTRY_VERIFY(!confirm->property("visible").toBool());
        controller.recoveries()->replace({{{"id", "committer-route"}, {"epoch", 9}, {"committerAvailable", true}}});
        QTest::qWait(50);
        auto *unavailable = findItem(item, "recoverEncryptedChannel");
        QVERIFY(unavailable);
        QVERIFY(!unavailable->property("visible").toBool());
        controller.m_state.insert("canRecover", false); emit controller.stateChanged();
        QTest::qWait(50);
        QVERIFY(!findItem(item, "recoverEncryptedChannel"));
        controller.m_recoveries.replace({{{"id", "private"}}});
        controller.hideWhileLocking();
        QCOMPARE(controller.recoveries()->rowCount(), 0);
    }
    void profileRolePresentationTracksModelChanges() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AccountCard.qml"));
        QScopedPointer<QObject> card(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}, {"account", "person"}}));
        QVERIFY2(card, qPrintable(component.errorString()));
        controller.members()->replace({{{"id", "person"}, {"roles", QStringList{"low", "high"}}}});
        controller.roles()->replace({{{"id", "low"}, {"name", "Low"}, {"priority", 1}}, {{"id", "high"}, {"name", "High"}, {"priority", 9}}});
        auto roles = [&card] { return card->property("visibleRoles").value<QJSValue>().toVariant().toList(); };
        QTRY_COMPARE(roles().size(), 2);
        QCOMPARE(roles().first().toMap().value("id").toString(), "high");
        controller.roles()->replace({{{"id", "low"}, {"name", "Renamed"}, {"color", "#ffffff"}, {"priority", 12}}, {{"id", "high"}, {"name", "High"}, {"priority", 9}}});
        QTRY_COMPARE(roles().first().toMap().value("name").toString(), "Renamed");
        QCOMPARE(roles().first().toMap().value("color").toString(), "#ffffff");
        controller.members()->replace({{{"id", "person"}, {"roles", QStringList{"high"}}}});
        QTRY_COMPARE(roles().size(), 1);
        QCOMPARE(roles().first().toMap().value("id").toString(), "high");
        controller.roles()->replace({});
        QTRY_VERIFY(roles().isEmpty());
    }
    void profileRoleControlsRequireFreshExactTargetActions() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_busy = true;
        controller.m_state = {{"account", "actor"}, {"server", "server"}, {"revision", 9}, {"rolesActive", true},
            {"profile", QVariantMap{{"account", "person"}, {"name", "Profile preview"}}},
            {"roleTarget", QVariantMap{{"account", "person"}, {"revision", 9}, {"loaded", true}, {"assigned", QStringList{"low"}}, {"assignable", QStringList{"high"}}, {"removable", QStringList{"low"}}}}};
        controller.roles()->replace({{{"id", "high"}, {"name", "Helpers"}, {"priority", 20}, {"color", "#88ccaa"}},
            {{"id", "low"}, {"name", "An exceptionally long role name that should wrap safely in a narrow profile"}, {"priority", 10}, {"color", "#88aacc"}}});
        QQmlEngine engine;
        QQuickWindow window; window.resize(760, 720); window.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/AccountCard.qml"));
        QScopedPointer<QObject> card(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}, {"account", "person"}, {"parent", QVariant::fromValue(window.contentItem())}}));
        QVERIFY2(card, qPrintable(component.errorString()));
        QVERIFY(QMetaObject::invokeMethod(card.data(), "open"));
        auto *add = card->findChild<QQuickItem *>("profileAddRole");
        QVERIFY(add);
        QTRY_VERIFY(add->isVisible());
        QVERIFY(!add->isEnabled());
        auto *selector = card->findChild<QObject *>("profileRoleSelector");
        QVERIFY(selector);
        controller.m_busy = false; emit controller.busyChanged();
        QTRY_VERIFY(add->isEnabled());
        QVERIFY(QMetaObject::invokeMethod(add, "clicked"));
        QTRY_VERIFY(selector->property("visible").toBool());
        auto *search = card->findChild<QObject *>("profileRoleSearch");
        QVERIFY(search);
        search->setProperty("text", "Helpers");
        QCOMPARE(card->property("availableRoles").value<QJSValue>().toVariant().toList().size(), 1);
        QVERIFY(QMetaObject::invokeMethod(selector, "close"));
        QTRY_VERIFY(card->property("opened").toBool());
        QTest::qWait(150);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/profile-inline-roles.png"));
        auto target = controller.m_state.value("roleTarget").toMap();
        target["assignable"] = QStringList{}; target["removable"] = QStringList{};
        controller.m_state["roleTarget"] = target; emit controller.stateChanged();
        QTRY_VERIFY(!add->isVisible());
        QVERIFY(card->property("roleActionsLoaded").toBool());
        QCOMPARE(card->property("visibleRoles").value<QJSValue>().toVariant().toList().size(), 1);
        target["assignable"] = QStringList{"high"}; target["removable"] = QStringList{"low"};
        controller.m_state["roleTarget"] = target; controller.m_state["revision"] = 10; emit controller.stateChanged();
        QTRY_VERIFY(!card->property("roleActionsLoaded").toBool());
        QVERIFY(!add->isVisible());
        controller.m_state["revision"] = 9; target["account"] = "different-person";
        controller.m_state["roleTarget"] = target; emit controller.stateChanged();
        QVERIFY(!card->property("roleActionsLoaded").toBool());
        QVERIFY(!add->isVisible());
        controller.m_state["server"] = "other"; emit controller.stateChanged();
        QTRY_VERIFY(!card->property("visible").toBool());
        QVERIFY(!selector->property("visible").toBool());
        // A full many-role card must remain bounded and scroll on a small window.
        QList<QVariantMap> many;
        QStringList assigned;
        for (int i = 0; i < 32; ++i) {
            const auto id = QString("role-%1").arg(i);
            assigned.push_back(id);
            many.push_back({{"id", id}, {"name", i % 3 ? QString("Team %1").arg(i) : QString("An exceptionally long community role label %1").arg(i)},
                {"priority", i}, {"color", i % 2 ? "#88bbcc" : "#bb88cc"}});
        }
        controller.m_busy = true; emit controller.busyChanged();
        controller.roles()->replace(many);
        controller.m_state["roleTarget"] = QVariantMap{{"loaded", true}, {"account", "person"}, {"revision", 9}, {"assigned", assigned}, {"assignable", QStringList{}}, {"removable", QStringList{}}};
        emit controller.stateChanged();
        window.resize(380, 520);
        QVERIFY(QMetaObject::invokeMethod(card.data(), "open"));
        QTRY_VERIFY(card->property("opened").toBool());
        QTRY_COMPARE(card->property("visibleRoles").value<QJSValue>().toVariant().toList().size(), 32);
        QCOMPARE(card->property("visibleRoles").value<QJSValue>().toVariant().toList().first().toMap().value("id").toString(), "role-31");
        QTRY_VERIFY(card->property("contentHeight").toReal() > card->property("height").toReal());
        QVERIFY(card->property("height").toReal() <= window.height() - 24);
        QVERIFY(card->property("width").toReal() <= window.width() - 24);
        QTest::qWait(150);
        QVERIFY(window.grabWindow().save(QStringLiteral(CORDS_TEST_OUTPUT) + "/profile-many-roles-narrow.png"));
    }
    void roleEditorFollowsReviewedAccountAndServer() {
        QTemporaryDir state;
        Controller controller(state.path(), QStringLiteral(CORDS_TEST_MIGRATIONS), {});
        QTRY_VERIFY_WITH_TIMEOUT(!controller.state().isEmpty(), 5000);
        controller.m_poll.stop();
        controller.m_state = {{"canCreateRoles", true}, {"canRoles", true}, {"revision", 7}, {"account", "first-account"}, {"server", "first-server"}};
        QQmlEngine engine;
        QQuickWindow window; window.resize(800, 780); window.show();
        QQmlComponent component(&engine, QUrl::fromLocalFile(QStringLiteral(CORDS_QML_DIR) + "/RoleEditor.qml"));
        QScopedPointer<QObject> editor(component.createWithInitialProperties({{"desktop", QVariant::fromValue(&controller)}, {"parent", QVariant::fromValue(window.contentItem())}}));
        QVERIFY2(editor, qPrintable(component.errorString()));
        controller.roles()->replace({{{"id", "custom-role"}, {"editable", true}, {"deletable", true}}});
        const QVariantMap role{{"id", "custom-role"}, {"name", "Guides"}, {"color", "#80b5db"}, {"permissions", QStringList{}}};
        for (const auto &scopeKey : {QString("account"), QString("server")}) {
            for (const auto &confirmationName : {QString("deleteRoleConfirmation"), QString("discardRoleConfirmation")}) {
                QVERIFY(QMetaObject::invokeMethod(editor.data(), "edit", Q_ARG(QVariant, role), Q_ARG(QVariant, true)));
                QTRY_VERIFY(editor->property("visible").toBool());
                QVERIFY(editor->property("canEdit").toBool());
                auto *confirmation = editor->findChild<QObject *>(confirmationName);
                QVERIFY(confirmation);
                QVERIFY(confirmation->property("operation").value<QJSValue>().isCallable());
                QVERIFY(QMetaObject::invokeMethod(confirmation, "open"));
                QTRY_VERIFY(confirmation->property("visible").toBool());
                controller.m_state[scopeKey] = controller.m_state[scopeKey].toString() + "-changed";
                emit controller.stateChanged();
                QTRY_VERIFY(!editor->property("visible").toBool());
                QTRY_VERIFY(!confirmation->property("visible").toBool());
                QVERIFY(!editor->property("canEdit").toBool());
                QVERIFY(editor->property("draft").value<QJSValue>().toVariant().toMap().isEmpty());
                QVERIFY(!confirmation->property("operation").value<QJSValue>().isCallable());
            }
        }
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "edit", Q_ARG(QVariant, role), Q_ARG(QVariant, true)));
        QTRY_VERIFY(editor->property("visible").toBool());
        controller.m_state["revision"] = 8;
        emit controller.stateChanged();
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "save"));
        QVERIFY(!editor->property("submitting").toBool());
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "removeRole"));
        QVERIFY(!editor->property("submitting").toBool());
        auto builtin = role;
        builtin["id"] = "builtin:default";
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "edit", Q_ARG(QVariant, builtin), Q_ARG(QVariant, true)));
        QVERIFY(editor->findChild<QObject *>("roleName")->property("readOnly").toBool());
        QVERIFY(!editor->findChild<QObject *>("rolePriority")->property("enabled").toBool());
        QVERIFY(!editor->findChild<QObject *>("deleteRole")->property("visible").toBool());
        QVERIFY(QMetaObject::invokeMethod(editor.data(), "close"));
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
        // The crop must retain a pointer drag even when its enclosing dialog
        // scrolls. Exercise actual pointer events, not only property changes.
        auto *previewItem = qobject_cast<QQuickItem *>(preview);
        QVERIFY(previewItem);
        window.resize(680, 520);
        window.raise(); window.requestActivate();
        QTest::qWait(100);
        for (const double scale : {2.0, 4.0}) {
            zoom->setProperty("value", scale);
            editor->setProperty("centerX", 0.5);
            editor->setProperty("centerY", 0.5);
            const auto start = previewItem->mapToScene(QPointF(previewItem->width() / 2, previewItem->height() / 2)).toPoint();
            QTest::mousePress(&window, Qt::LeftButton, Qt::NoModifier, start);
            for (int step = 1; step <= 10; ++step)
                QTest::mouseMove(&window, start + QPoint(step * 8, step * 6), 20);
            QTest::mouseRelease(&window, Qt::LeftButton, Qt::NoModifier, start + QPoint(80, 60));
            const double expectedX = 0.5 - 80.0 / previewItem->width() / scale;
            const double expectedY = 0.5 - 60.0 / previewItem->height() / scale;
            QVERIFY2(qAbs(editor->property("centerX").toDouble() - expectedX) < 0.01, "crop must follow the entire horizontal drag");
            QVERIFY2(qAbs(editor->property("centerY").toDouble() - expectedY) < 0.01, "crop must follow the entire vertical drag");
        }
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
