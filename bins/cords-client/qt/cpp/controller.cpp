#include "controller.h"
#include <QClipboard>
#include <QGuiApplication>
#include <QUuid>

namespace {
rust::String rs(const QString &text) { const auto bytes = text.toUtf8(); return {bytes.constData(), static_cast<size_t>(bytes.size())}; }
QString qs(const rust::String &text) { return QString::fromUtf8(text.data(), static_cast<qsizetype>(text.size())); }
QStringList strings(const rust::Vec<rust::String> &values) { QStringList result; for (const auto &value : values) result.push_back(qs(value)); return result; }
rust::Vec<rust::String> rustStrings(const QStringList &values) { rust::Vec<rust::String> result; for (const auto &value : values) result.push_back(rs(value)); return result; }
template<class RowsType, class Convert> QList<QVariantMap> rows(const RowsType &values, Convert convert) {
    QList<QVariantMap> result; result.reserve(static_cast<qsizetype>(values.size()));
    for (const auto &value : values) result.push_back(convert(value)); return result;
}
QVariantMap profile(const cords::Profile &p) { return {{"account", qs(p.account)}, {"name", qs(p.name)}, {"image", qs(p.image)}, {"shape", qs(p.shape)}, {"heads", strings(p.heads)}, {"conflicts", strings(p.conflicts)}, {"pending", qs(p.pending)}}; }
QVariantMap preferences(const cords::Preferences &p) { return {{"name", qs(p.name)}, {"theme", qs(p.theme)}, {"compact", p.compact}, {"reducedMotion", p.reduced_motion}, {"disableAnimations", p.disable_animations}, {"showMembers", p.show_members}, {"autoLockMinutes", p.auto_lock_minutes}, {"genericize", p.genericize}, {"hideName", p.hide_name}, {"lockOnSuspend", p.lock_on_suspend}, {"lockOnOsLock", p.lock_on_os_lock}, {"notifications", p.notifications}, {"notificationPreviews", p.notification_previews}, {"doNotDisturb", p.do_not_disturb}}; }
}

Controller::Controller(QString statePath, QString migrations, QString ca, QObject *parent)
    : QObject(parent), m_desktop(cords::start_desktop(rs(statePath), rs(migrations), rs(ca))) {
    connect(&m_poll, &QTimer::timeout, this, &Controller::poll); m_poll.start(80);
}
void Controller::poll() {
    try {
        for (const auto &snapshot : m_desktop->take_updates()) apply(snapshot);
        for (const auto &alert : m_desktop->take_alerts())
            if (!m_locking && m_state.value("unlocked").toBool()) emit notificationApproved(qs(alert.title), qs(alert.body));
    }
    catch (const std::exception &error) { m_state["error"] = QString::fromUtf8(error.what()); emit stateChanged(); }
}
void Controller::submit(const std::function<void()> &operation) {
    if (m_busy) { m_state["error"] = tr("An operation is still running. Wait for its result before retrying."); emit stateChanged(); return; }
    try { operation(); m_busy = true; emit busyChanged(); }
    catch (const std::exception &error) { m_state["error"] = QString::fromUtf8(error.what()); emit stateChanged(); }
}
void Controller::apply(const cords::Snapshot &s) {
    if (m_locking && s.unlocked) return;
    if (!s.unlocked) m_locking = false;
    m_accounts.replace(rows(s.accounts, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"name", qs(r.name)}, {"avatar", qs(r.avatar)}, {"shape", qs(r.shape)}, {"xMilli", r.x_milli}, {"yMilli", r.y_milli}, {"zoomMilli", r.zoom_milli}}; }));
    m_servers.replace(rows(s.servers, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"name", qs(r.origin)}, {"origin", qs(r.origin)}, {"state", qs(r.state)}, {"selected", r.selected}, {"archived", r.archived}, {"connected", r.connected}, {"error", qs(r.error)}, {"unread", r.unread}}; }));
    m_channels.replace(rows(s.channels, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"name", qs(r.name)}, {"mode", qs(r.mode)}, {"epoch", QVariant::fromValue(r.epoch)}, {"joined", r.joined}, {"retired", r.retired}, {"acknowledgement", r.acknowledgement}, {"canManage", r.can_manage}}; }));
    m_messages.replace(rows(s.messages, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"account", qs(r.account)}, {"author", qs(r.author)}, {"body", qs(r.body)}, {"timestamp", QVariant::fromValue(r.timestamp)}}; }));
    m_members.replace(rows(s.members, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"name", qs(r.name)}, {"roles", strings(r.roles)}, {"group", qs(r.group)}, {"color", qs(r.color)}}; }));
    m_devices.replace(rows(s.devices, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"account", qs(r.account)}, {"name", qs(r.name)}, {"generation", QVariant::fromValue(r.generation)}}; }));
    m_requests.replace(rows(s.requests, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"account", qs(r.account)}, {"name", qs(r.name)}, {"status", qs(r.state)}}; }));
    m_roles.replace(rows(s.roles, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"name", qs(r.name)}, {"color", qs(r.color)}, {"description", qs(r.description)}, {"priority", r.priority}, {"permissions", strings(r.permissions)}, {"mentionable", r.mentionable}}; }));
    m_audit.replace(rows(s.audit, [](const auto &r) -> QVariantMap { return {{"id", QVariant::fromValue(r.revision)}, {"actor", qs(r.actor)}, {"operation", qs(r.operation)}, {"target", qs(r.target)}, {"timestamp", QVariant::fromValue(r.timestamp)}}; }));
    m_notifications.replace(rows(s.notifications, [](const auto &r) -> QVariantMap { return {{"id", qs(r.id)}, {"label", qs(r.label)}, {"body", qs(r.body)}}; }));
    QVariantMap people;
    for (const auto &person : s.people)
        people.insert(qs(person.id), QVariantMap{{"image", qs(person.image)}, {"shape", qs(person.shape)}});
    m_state = {{"people", people}, {"presentationError", qs(s.presentation_error)}, {"directoryLoaded", s.directory_loaded},
        {"unlocked", s.unlocked}, {"legacy", s.legacy}, {"account", qs(s.account)}, {"device", qs(s.device)},
        {"server", qs(s.server)}, {"origin", qs(s.origin)}, {"ownership", qs(s.ownership)}, {"admission", qs(s.admission)},
        {"route", qs(s.route)}, {"error", qs(s.error)}, {"notice", qs(s.notice)}, {"connected", s.connected},
        {"canCreate", s.can_create}, {"canApprove", s.can_approve}, {"canRoles", s.can_roles}, {"canAssign", s.can_assign},
        {"canAudit", s.can_audit}, {"revoked", s.revoked}, {"burned", s.burned}, {"revision", QVariant::fromValue(s.revision)},
        {"rolesActive", s.roles_active}, {"directoryAfter", qs(s.directory_after)}, {"auditAfter", QVariant::fromValue(s.audit_after)},
        {"profile", profile(s.profile)}, {"preferences", preferences(s.preferences)}, {"platformWarning", m_platformWarning},
        {"serverMuted", s.server_muted}, {"routeMuted", s.route_muted}, {"notificationStatus", m_notificationStatus}};
    if (s.completed != m_completed) { m_completed = s.completed; m_busy = false; emit busyChanged(); }
    emit stateChanged();
}
// These wrappers only marshal typed input and queue work. Rust owns all decisions.
void Controller::unlockStdin(QString account) { m_desktop->unlock_stdin(rs(account)); m_busy = true; emit busyChanged(); }
#define SIMPLE(camel, snake) void Controller::camel() { submit([&] { m_desktop->snake(); }); }
#define ONE(camel, snake) void Controller::camel(QString value) { submit([&] { m_desktop->snake(rs(value)); }); }
#define TWO(camel, snake) void Controller::camel(QString first, QString second) { submit([&] { m_desktop->snake(rs(first), rs(second)); }); }
SIMPLE(refresh, refresh) SIMPLE(authenticate, authenticate) SIMPLE(burnIdentity, burn_identity)
SIMPLE(retryBurns, retry_burns) SIMPLE(publishKeys, publish_keys) SIMPLE(synchronize, synchronize)
SIMPLE(loadRequests, load_requests) SIMPLE(revokeDevice, revoke_device) SIMPLE(loadRoles, load_roles)
ONE(trustServer, trust_server) ONE(selectServer, select_server) ONE(claimOwnership, claim_ownership)
ONE(recoverOwnership, recover_ownership) ONE(designateSuccessor, designate_successor) ONE(acceptSuccessor, accept_successor)
ONE(archiveServer, archive_server) ONE(selectChannel, select_channel) ONE(retireChannel, retire_channel)
ONE(acknowledgePublic, acknowledge_public) ONE(joinChannel, join_channel) ONE(loadDirectory, load_directory)
ONE(approveRequest, approve_request) ONE(rejectRequest, reject_request) ONE(loadAccountRoles, load_account_roles)
ONE(loadProfile, load_profile) TWO(unlockAccount, unlock_account) TWO(removeAccount, remove_account)
TWO(sendMessage, send_message) TWO(addDevice, add_device) TWO(removeDevice, remove_device)
#undef SIMPLE
#undef ONE
#undef TWO
void Controller::createAccount(QString name, QString password, bool weak) { submit([&] { m_desktop->create_account(rs(name), rs(password), weak); }); }
void Controller::migrateAccount(QString name, QString old, QString password, bool weak) { submit([&] { m_desktop->migrate_account(rs(name), rs(old), rs(password), weak); }); }
void Controller::hideWhileLocking() {
    m_locking = true;
    m_busy = true;
    m_servers.replace({}); m_channels.replace({}); m_messages.replace({});
    m_members.replace({}); m_devices.replace({}); m_requests.replace({});
    m_roles.replace({}); m_audit.replace({}); m_notifications.replace({});
    m_state = {{"unlocked", false}, {"legacy", m_state.value("legacy")}, {"notice", tr("Locking account…")}, {"platformWarning", m_platformWarning}};
    emit busyChanged(); emit stateChanged();
}
void Controller::lockAccount(bool signOut) {
    if (m_locking) return;
    try { m_desktop->lock_account(signOut); hideWhileLocking(); }
    catch (const std::exception &error) { m_state["error"] = QString::fromUtf8(error.what()); emit stateChanged(); }
}
void Controller::platformLock(bool suspended) {
    try { if (m_desktop->platform_lock(suspended)) hideWhileLocking(); }
    catch (const std::exception &error) { m_state["error"] = QString::fromUtf8(error.what()); emit stateChanged(); }
}
void Controller::platformWarning(QString message) {
    m_platformWarning = std::move(message);
    m_state["platformWarning"] = m_platformWarning;
    emit stateChanged();
}
void Controller::notificationEnvironment(bool visible, bool allowed, QString status) {
    m_desktop->notification_environment(visible, allowed);
    if (status != m_notificationStatus) {
        m_notificationStatus = std::move(status);
        m_state["notificationStatus"] = m_notificationStatus;
        emit stateChanged();
    }
}
void Controller::activateAlert(QString id) {
    submit([&] { m_desktop->activate_alert(rs(id)); });
    emit conversationRequested();
}
void Controller::muteServer(bool muted) { submit([&] { m_desktop->mute_server(muted); }); }
void Controller::muteRoute(QString route, bool muted) { submit([&] { m_desktop->mute_route(rs(route), muted); }); }
void Controller::openNotifications() {
    if (!m_state.value("unlocked").toBool()) return;
    if (m_notifications.rowCount() == 1) activateAlert(m_notifications.get(0).value("id").toString());
    else emit notificationsRequested();
}
void Controller::changePassword(QString account, QString old, QString password, bool weak) { submit([&] { m_desktop->change_password(rs(account), rs(old), rs(password), weak); }); }
void Controller::removeServer(QString server, bool local) { submit([&] { m_desktop->remove_server(rs(server), local); }); }
void Controller::createChannel(QString name, bool isPublic) { submit([&] { m_desktop->create_channel(rs(name), isPublic); }); }
void Controller::replaceChannel(QString channel, QString name, bool isPublic) { submit([&] { m_desktop->replace_channel(rs(channel), rs(name), isPublic); }); }
void Controller::loadAudit(quint64 after) { submit([&] { m_desktop->load_audit(after); }); }
void Controller::putRole(QVariantMap value, quint64 revision, QString requestId) {
    submit([&] { m_desktop->put_role(cords::RoleRow{rs(value["id"].toString()), rs(value["name"].toString()), rs(value["color"].toString()), rs(value["description"].toString()),
        value["priority"].toUInt(), rustStrings(value["permissions"].toStringList()), value["mentionable"].toBool()}, revision, rs(requestId)); });
}
void Controller::deleteRole(QString role, quint64 revision, QString requestId) { submit([&] { m_desktop->delete_role(rs(role), revision, rs(requestId)); }); }
void Controller::assignRole(QString account, QString role, bool remove, quint64 revision, QString requestId) { submit([&] { m_desktop->assign_role(rs(account), rs(role), remove, revision, rs(requestId)); }); }
void Controller::saveProfileName(QString name, QStringList heads) { submit([&] { m_desktop->save_profile_name(rs(name), rustStrings(heads)); }); }
void Controller::savePreferences(QVariantMap p) {
    submit([&] { m_desktop->save_preferences(cords::Preferences{rs(p["name"].toString()), rs(p["theme"].toString()), p["compact"].toBool(), p["reducedMotion"].toBool(),
        p["disableAnimations"].toBool(), p["showMembers"].toBool(), p["autoLockMinutes"].toUInt(), p["genericize"].toBool(), p["hideName"].toBool(), p["lockOnSuspend"].toBool(), p["lockOnOsLock"].toBool(), p["notifications"].toBool(), p["notificationPreviews"].toBool(), p["doNotDisturb"].toBool()}); });
}
void Controller::copy(QString text) { QGuiApplication::clipboard()->setText(text); }
QString Controller::newId() const { return QUuid::createUuid().toString(QUuid::WithoutBraces); }
void Controller::recordActivity() { m_desktop->activity(); }
