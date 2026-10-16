#pragma once
#include <QAbstractNativeEventFilter>
#include <QtGlobal>

class Controller;
class QQuickWindow;

// Native event translation only. Rust evaluates account lock preferences.
class PlatformEvents final : public QAbstractNativeEventFilter {
public:
    PlatformEvents(Controller &controller, QQuickWindow &window);
    ~PlatformEvents() override;
    bool nativeEventFilter(const QByteArray &eventType, void *message, qintptr *result) override;
private:
    Controller &m_controller;
    quintptr m_handle = 0;
    bool m_registered = false;
};
