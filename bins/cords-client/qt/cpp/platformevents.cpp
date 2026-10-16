#include "platformevents.h"
#include "controller.h"
#include <QCoreApplication>
#include <QQuickWindow>
#ifdef Q_OS_WIN
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <wtsapi32.h>
#endif

PlatformEvents::PlatformEvents(Controller &controller, QQuickWindow &window)
    : m_controller(controller), m_handle(window.winId()) {
#ifdef Q_OS_WIN
    m_registered = WTSRegisterSessionNotification(reinterpret_cast<HWND>(m_handle), NOTIFY_FOR_THIS_SESSION);
    if (!m_registered)
        m_controller.platformWarning(QStringLiteral("Windows session-lock events are unavailable (error %1). Use manual locking.").arg(GetLastError()));
    QCoreApplication::instance()->installNativeEventFilter(this);
#else
    m_controller.platformWarning(QStringLiteral("OS session-lock events are not connected on this platform. Manual and inactivity locking remain available."));
#endif
}
PlatformEvents::~PlatformEvents() {
    QCoreApplication::instance()->removeNativeEventFilter(this);
#ifdef Q_OS_WIN
    if (m_registered && !WTSUnRegisterSessionNotification(reinterpret_cast<HWND>(m_handle)))
        qWarning("Could not unregister Windows session-lock events: %lu", GetLastError());
#endif
}
bool PlatformEvents::nativeEventFilter(const QByteArray &eventType, void *message, qintptr *result) {
    Q_UNUSED(eventType)
    Q_UNUSED(result)
#ifdef Q_OS_WIN
    const auto *event = static_cast<const MSG *>(message);
    if (event->hwnd != reinterpret_cast<HWND>(m_handle)) return false;
    if (event->message == WM_WTSSESSION_CHANGE && event->wParam == WTS_SESSION_LOCK)
        m_controller.platformLock(false);
    else if (event->message == WM_POWERBROADCAST && event->wParam == PBT_APMSUSPEND)
        m_controller.platformLock(true);
#else
    Q_UNUSED(message)
#endif
    return false;
}
