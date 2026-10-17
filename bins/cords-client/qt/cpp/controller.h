#pragma once
#include "rows.h"
#include "cords-qt-bridge/src/lib.rs.h"
#include <QTimer>
#include <functional>
#include <QtQml/qqmlregistration.h>

class Controller : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("The application owns the account runtime")
    Q_PROPERTY(QVariantMap state READ state NOTIFY stateChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(Rows *accounts READ accounts CONSTANT)
    Q_PROPERTY(Rows *servers READ servers CONSTANT)
    Q_PROPERTY(Rows *channels READ channels CONSTANT)
    Q_PROPERTY(Rows *messages READ messages CONSTANT)
    Q_PROPERTY(Rows *members READ members CONSTANT)
    Q_PROPERTY(Rows *devices READ devices CONSTANT)
    Q_PROPERTY(Rows *requests READ requests CONSTANT)
    Q_PROPERTY(Rows *roles READ roles CONSTANT)
    Q_PROPERTY(Rows *audit READ audit CONSTANT)
    Q_PROPERTY(Rows *notifications READ notifications CONSTANT)
public:
    Controller(QString statePath, QString migrations, QString ca, QObject *parent = nullptr);
    QVariantMap state() const { return m_state; }
    bool busy() const { return m_busy; }
    Rows *accounts() { return &m_accounts; } Rows *servers() { return &m_servers; }
    Rows *channels() { return &m_channels; } Rows *messages() { return &m_messages; }
    Rows *members() { return &m_members; } Rows *devices() { return &m_devices; }
    Rows *requests() { return &m_requests; } Rows *roles() { return &m_roles; }
    Rows *audit() { return &m_audit; }
    Rows *notifications() { return &m_notifications; }
    Q_INVOKABLE void refresh();
    Q_INVOKABLE void createAccount(QString name, QString password, bool allowWeak);
    Q_INVOKABLE void unlockAccount(QString account, QString password);
    void unlockStdin(QString account);
    Q_INVOKABLE void migrateAccount(QString name, QString oldPassword, QString password, bool allowWeak);
    Q_INVOKABLE void lockAccount(bool signOut = false);
    Q_INVOKABLE void removeAccount(QString account, QString password);
    Q_INVOKABLE void changePassword(QString account, QString oldPassword, QString password, bool allowWeak);
    Q_INVOKABLE void trustServer(QString origin);
    Q_INVOKABLE void selectServer(QString server);
    Q_INVOKABLE void authenticate();
    Q_INVOKABLE void claimOwnership(QString code);
    Q_INVOKABLE void recoverOwnership(QString code);
    Q_INVOKABLE void designateSuccessor(QString account);
    Q_INVOKABLE void acceptSuccessor(QString hash);
    Q_INVOKABLE void removeServer(QString server, bool localOnly);
    Q_INVOKABLE void archiveServer(QString server);
    Q_INVOKABLE void burnIdentity();
    Q_INVOKABLE void retryBurns();
    Q_INVOKABLE void selectChannel(QString channel);
    Q_INVOKABLE void olderMessages(QString channel);
    Q_INVOKABLE void latestMessages(QString channel);
    Q_INVOKABLE void createChannel(QString name, bool isPublic);
    Q_INVOKABLE void replaceChannel(QString channel, QString name, bool isPublic);
    Q_INVOKABLE void retireChannel(QString channel);
    Q_INVOKABLE void acknowledgePublic(QString channel);
    Q_INVOKABLE void joinChannel(QString channel);
    Q_INVOKABLE void publishKeys();
    Q_INVOKABLE void sendMessage(QString channel, QString body);
    Q_INVOKABLE void synchronize();
    Q_INVOKABLE void loadDirectory(QString after = {});
    Q_INVOKABLE void loadRequests();
    Q_INVOKABLE void approveRequest(QString device);
    Q_INVOKABLE void rejectRequest(QString device);
    Q_INVOKABLE void addDevice(QString channel, QString device);
    Q_INVOKABLE void removeDevice(QString channel, QString device);
    Q_INVOKABLE void revokeDevice();
    Q_INVOKABLE void loadRoles();
    Q_INVOKABLE void loadAccountRoles(QString account);
    Q_INVOKABLE void putRole(QVariantMap role, quint64 revision, QString requestId);
    Q_INVOKABLE void deleteRole(QString role, quint64 revision, QString requestId);
    Q_INVOKABLE void assignRole(QString account, QString role, bool remove, quint64 revision, QString requestId);
    Q_INVOKABLE void loadAudit(quint64 after = 0);
    Q_INVOKABLE void loadProfile(QString account);
    Q_INVOKABLE void saveProfileName(QString name, QStringList heads);
    Q_INVOKABLE void savePreferences(QVariantMap preferences);
    Q_INVOKABLE void copy(QString text);
    Q_INVOKABLE QString newId() const;
    Q_INVOKABLE void recordActivity();
    void poll();
    void platformLock(bool suspended);
    void platformWarning(QString message);
    void notificationEnvironment(bool visible, bool allowed, QString status);
    void openNotifications();
    Q_INVOKABLE void activateAlert(QString id);
    Q_INVOKABLE void muteServer(bool muted);
    Q_INVOKABLE void muteRoute(QString route, bool muted);
signals:
    void stateChanged();
    void busyChanged();
    void notificationApproved(QString title, QString body);
    void notificationsRequested();
    void conversationRequested();
private:
    friend class ControllerTest;
    void submit(const std::function<void()> &operation);
    void apply(const cords::Snapshot &snapshot);
    void hideWhileLocking();
    rust::Box<cords::Desktop> m_desktop;
    QTimer m_poll;
    QVariantMap m_state;
    QString m_platformWarning;
    QString m_notificationStatus;
    Rows m_accounts, m_servers, m_channels, m_messages, m_members, m_devices, m_requests, m_roles, m_audit, m_notifications;
    bool m_busy = false;
    bool m_locking = false;
    quint64 m_completed = 0;
};
