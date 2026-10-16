#pragma once
#include <QMenu>
#include <QObject>
#include <QSystemTrayIcon>
#include <QTimer>

class Controller;
class QQuickWindow;

// Only presents Rust-approved text. Native clicks open pending activity; the
// tray API cannot identify individual historical OS notification clicks.
class NativeNotifications final : public QObject {
public:
    NativeNotifications(Controller &controller, QQuickWindow &window);
private:
    void updateEnvironment();
    bool platformAllows(QString *status = nullptr) const;
    void restore();
    Controller &m_controller;
    QQuickWindow &m_window;
    QMenu m_menu;
    QSystemTrayIcon m_tray;
    QTimer m_timer;
};
