#include "notifications.h"
#include "controller.h"
#include <QCoreApplication>
#include <QGuiApplication>
#include <QQuickWindow>
#ifdef Q_OS_WIN
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <shellapi.h>
#endif

NativeNotifications::NativeNotifications(Controller &controller, QQuickWindow &window)
    : m_controller(controller), m_window(window), m_tray(QIcon(":/128x128.png")) {
    m_tray.setToolTip(QStringLiteral("Cords"));
    m_menu.addAction(tr("Open Cords"), this, [this] { restore(); });
    m_menu.addAction(tr("Lock account"), this, [this] { m_controller.lockAccount(false); });
    m_menu.addSeparator();
    m_menu.addAction(tr("Quit"), QCoreApplication::instance(), &QCoreApplication::quit);
    m_tray.setContextMenu(&m_menu);
    connect(&m_tray, &QSystemTrayIcon::messageClicked, this, [this] {
        restore(); m_controller.openNotifications();
    });
    connect(&m_tray, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) restore();
    });
    connect(&m_controller, &Controller::notificationApproved, this, [this](const QString &title, const QString &body) {
        if (m_controller.state().value("unlocked").toBool() && platformAllows())
            m_tray.showMessage(title, body, QSystemTrayIcon::Information, 5000);
    });
    connect(&m_controller, &Controller::stateChanged, this, [this] {
        const auto state = m_controller.state();
        const bool enabled = state.value("preferences").toMap().value("notifications").toBool();
        m_tray.setVisible(state.value("unlocked").toBool() && enabled && QSystemTrayIcon::isSystemTrayAvailable());
    });
    connect(&m_timer, &QTimer::timeout, this, [this] { updateEnvironment(); });
    connect(&m_window, &QWindow::activeChanged, this, [this] { updateEnvironment(); });
    connect(&m_window, &QWindow::visibilityChanged, this, [this] { updateEnvironment(); });
    m_timer.start(500);
    updateEnvironment();
}
bool NativeNotifications::platformAllows(QString *status) const {
    auto unavailable = [status](const QString &reason) {
        if (status) *status = reason;
        return false;
    };
    if (!QSystemTrayIcon::isSystemTrayAvailable())
        return unavailable(tr("The desktop notification area is unavailable."));
    if (!QSystemTrayIcon::supportsMessages())
        return unavailable(tr("The desktop notification area does not support alerts."));
#ifdef Q_OS_WIN
    QUERY_USER_NOTIFICATION_STATE state{};
    const HRESULT result = SHQueryUserNotificationState(&state);
    if (FAILED(result))
        return unavailable(tr("Windows notification state could not be checked (error %1).")
            .arg(static_cast<quint32>(result), 8, 16, QLatin1Char('0')));
    switch (state) {
    case QUNS_ACCEPTS_NOTIFICATIONS:
        if (status) *status = tr("Native presenter available; Windows may suppress alerts in its notification settings.");
        return true;
    case QUNS_NOT_PRESENT:
        return unavailable(tr("Windows is suppressing alerts while the user is away."));
    case QUNS_BUSY:
        return unavailable(tr("Windows reports that the user is busy; alerts are suppressed."));
    case QUNS_RUNNING_D3D_FULL_SCREEN:
        return unavailable(tr("Windows is suppressing alerts for a full-screen application."));
    case QUNS_PRESENTATION_MODE:
        return unavailable(tr("Windows presentation mode is suppressing alerts."));
    case QUNS_QUIET_TIME:
        return unavailable(tr("Windows quiet time is suppressing alerts."));
    default:
        return unavailable(tr("Windows is suppressing alerts (notification state %1).")
            .arg(static_cast<int>(state)));
    }
#else
    return unavailable(tr("Native notification privacy and Do Not Disturb integration is unavailable on this platform."));
#endif
}
void NativeNotifications::updateEnvironment() {
    QString status;
    const bool allowed = platformAllows(&status);
    const bool visible = m_window.isVisible() && m_window.visibility() != QWindow::Minimized && m_window.isActive();
    m_controller.notificationEnvironment(visible, allowed, status);
}
void NativeNotifications::restore() {
    if (m_window.visibility() == QWindow::Minimized) m_window.showNormal();
    m_window.show(); m_window.raise(); m_window.requestActivate();
}
