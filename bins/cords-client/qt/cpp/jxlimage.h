#pragma once
#include <QQuickPaintedItem>
#include <QImage>
#include <QTimer>
#include <QElapsedTimer>
#include <QtQml/qqmlregistration.h>
#include <memory>

struct JxlDecodeState;
class JxlImage : public QQuickPaintedItem {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString source READ source WRITE setSource NOTIFY sourceChanged)
    Q_PROPERTY(bool playing READ playing WRITE setPlaying NOTIFY playingChanged)
    Q_PROPERTY(bool square READ square WRITE setSquare NOTIFY squareChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY readyChanged)
    Q_PROPERTY(int frameNumber READ frameNumber NOTIFY frameChanged)
    Q_PROPERTY(qreal centerX MEMBER m_centerX NOTIFY cropChanged)
    Q_PROPERTY(qreal centerY MEMBER m_centerY NOTIFY cropChanged)
    Q_PROPERTY(qreal zoom MEMBER m_zoom NOTIFY cropChanged)
public:
    explicit JxlImage(QQuickItem *parent = nullptr);
    ~JxlImage() override;
    QString source() const { return m_source; }
    void setSource(const QString &source);
    bool playing() const { return m_playing; }
    void setPlaying(bool playing);
    bool square() const { return m_square; }
    void setSquare(bool square);
    QString error() const { return m_error; }
    bool ready() const { return !m_image.isNull(); }
    int frameNumber() const { return m_frame; }
    void paint(QPainter *painter) override;
    void pump();
signals:
    void sourceChanged();
    void playingChanged();
    void squareChanged();
    void errorChanged();
    void readyChanged();
    void frameChanged();
    void cropChanged();
private:
    void requestFrame();
    void decode();
    void suspend();
    bool eligible() const;
    QString m_source, m_error;
    bool m_playing = true, m_square = false, m_pending = false, m_queued = false, m_finished = false;
    int m_generation = 0, m_frame = 0;
    quint32 m_cycles = 0;
    QImage m_image;
    QTimer m_timer, m_visibility;
    QElapsedTimer m_clock;
    long double m_due = 0;
    qreal m_centerX = 0.5, m_centerY = 0.5, m_zoom = 1;
    std::shared_ptr<JxlDecodeState> m_decoder;
};
