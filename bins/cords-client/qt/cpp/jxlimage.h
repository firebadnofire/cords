#pragma once
#include <QQuickPaintedItem>
#include <QImage>
#include <QTimer>
#include <QElapsedTimer>
#include <QtQml/qqmlregistration.h>
#include <memory>

class JxlPlayback;
class JxlImage : public QQuickPaintedItem {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString source READ source WRITE setSource NOTIFY sourceChanged)
    Q_PROPERTY(QString account READ account WRITE setAccount NOTIFY accountChanged)
    Q_PROPERTY(bool playing READ playing WRITE setPlaying NOTIFY playingChanged)
    Q_PROPERTY(bool square READ square WRITE setSquare NOTIFY squareChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY readyChanged)
    Q_PROPERTY(bool animated READ animated NOTIFY animatedChanged)
    Q_PROPERTY(int frameNumber READ frameNumber NOTIFY frameChanged)
    Q_PROPERTY(qreal centerX MEMBER m_centerX NOTIFY cropChanged)
    Q_PROPERTY(qreal centerY MEMBER m_centerY NOTIFY cropChanged)
    Q_PROPERTY(qreal zoom MEMBER m_zoom NOTIFY cropChanged)
public:
    explicit JxlImage(QQuickItem *parent = nullptr);
    ~JxlImage() override;
    QString source() const { return m_source; }
    void setSource(const QString &source);
    QString account() const { return m_account; }
    void setAccount(const QString &account);
    bool playing() const { return m_playing; }
    void setPlaying(bool playing);
    bool square() const { return m_square; }
    void setSquare(bool square);
    QString error() const { return m_error; }
    bool ready() const { return !m_image.isNull(); }
    bool animated() const { return m_animated; }
    int frameNumber() const { return m_frame; }
    void paint(QPainter *painter) override;
signals:
    void sourceChanged();
    void accountChanged();
    void playingChanged();
    void squareChanged();
    void errorChanged();
    void readyChanged();
    void animatedChanged();
    void frameChanged();
    void cropChanged();
private:
    friend class JxlPlayback;
    friend class MediaTest;
    void requestFrame();
    void detach();
    void reset();
    bool eligible() const;
    QString m_source, m_account, m_error;
    bool m_playing = true, m_square = false, m_animated = false;
    int m_frame = 0;
    QImage m_image;
    QTimer m_visibility;
    qreal m_centerX = 0.5, m_centerY = 0.5, m_zoom = 1;
    std::shared_ptr<JxlPlayback> m_playback, m_stillPlayback;
};
