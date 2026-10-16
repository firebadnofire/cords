#include "jxlimage.h"
#include "cords-qt-bridge/src/lib.rs.h"
#include <QCoreApplication>
#include <QFutureWatcher>
#include <QBuffer>
#include <QImageReader>
#include <QPainter>
#include <QPainterPath>
#include <QPointer>
#include <QQuickWindow>
#include <QSemaphore>
#include <QtConcurrentRun>
#include <algorithm>
#include <optional>

namespace {
// Two bounded 128 MiB sandbox heaps plus host reservations and native transient
// canvases stay below 512 MiB. Completed cycles yield their slot in FIFO order.
QSemaphore decoderSlots(2);
QList<QPointer<JxlImage>> waiting;
QTimer *scheduler = nullptr;
int textureOwners = 0;
struct FrameResult {
    QImage image;
    QString error;
    quint32 ticks = 0, numerator = 1, denominator = 1, loops = 1;
    bool last = true;
};
}
struct JxlDecodeState {
    std::optional<rust::Box<cords::NativeImage>> image;
    ~JxlDecodeState() { image.reset(); decoderSlots.release(); }
};

JxlImage::JxlImage(QQuickItem *parent) : QQuickPaintedItem(parent) {
    ++textureOwners;
    setAntialiasing(true);
    m_timer.setSingleShot(true);
    connect(&m_timer, &QTimer::timeout, this, &JxlImage::requestFrame);
    connect(this, &JxlImage::cropChanged, this, [this] { update(); });
    connect(this, &QQuickItem::visibleChanged, this, [this] { if (eligible()) requestFrame(); else suspend(); });
    const auto observeWindow = [this](QQuickWindow *window) {
        if (window) {
            connect(window, &QWindow::visibilityChanged, this, [this] { if (eligible()) requestFrame(); else suspend(); });
            connect(window, &QWindow::visibleChanged, this, [this] { if (eligible()) requestFrame(); else suspend(); });
        }
        if (eligible()) requestFrame(); else suspend();
    };
    connect(this, &QQuickItem::windowChanged, this, observeWindow);
    observeWindow(window());
    m_visibility.setInterval(250);
    connect(&m_visibility, &QTimer::timeout, this, [this] {
        if (eligible()) requestFrame();
        else if (m_decoder || m_pending || m_queued) suspend();
    });
    m_visibility.start();
    if (!scheduler) {
        scheduler = new QTimer(QCoreApplication::instance());
        scheduler->setInterval(30);
        QObject::connect(scheduler, &QTimer::timeout, QCoreApplication::instance(), [] {
            while (!waiting.isEmpty()) {
                auto image = waiting.first();
                if (!image) { waiting.removeFirst(); continue; }
                if (decoderSlots.available() == 0) break;
                waiting.removeFirst(); image->pump();
            }
        });
        scheduler->start();
    }
}
JxlImage::~JxlImage() { suspend(); --textureOwners; }
bool JxlImage::eligible() const {
    if (!isVisible() || !window() || !window()->isVisible() || window()->visibility() == QWindow::Minimized
        || width() <= 0 || height() <= 0 || m_source.isEmpty()) return false;
    for (auto *parent = parentItem(); parent; parent = parent->parentItem()) {
        if (parent->clip() && !parent->boundingRect().intersects(mapRectToItem(parent, boundingRect()))) return false;
    }
    return QRectF(0, 0, window()->width(), window()->height()).intersects(mapRectToScene(boundingRect()));
}
void JxlImage::setSource(const QString &source) {
    if (m_source == source) return;
    suspend(); m_source = source; m_image = {}; m_error.clear(); m_finished = false; m_cycles = 0; m_frame = 0;
    if (textureOwners > 64 && !source.isEmpty()) m_error = tr("Visible media limit reached");
    emit sourceChanged(); emit readyChanged(); emit errorChanged(); emit frameChanged(); update();
    requestFrame();
}
void JxlImage::setPlaying(bool value) {
    if (m_playing == value) return;
    m_playing = value; emit playingChanged();
    if (value) { m_finished = false; requestFrame(); } else suspend();
}
void JxlImage::setSquare(bool value) { if (m_square != value) { m_square = value; emit squareChanged(); update(); } }
void JxlImage::suspend() {
    ++m_generation; m_timer.stop(); m_decoder.reset(); m_pending = false; m_queued = false;
    waiting.removeAll(this);
}
void JxlImage::requestFrame() {
    if (!eligible() || m_timer.isActive() || m_pending || m_queued || m_finished || (!m_playing && ready()) || !m_error.isEmpty()) return;
    if (m_decoder) { decode(); return; }
    if (waiting.size() >= 64) { m_error = tr("Visible media limit reached"); emit errorChanged(); return; }
    m_queued = true; waiting.append(this);
}
void JxlImage::pump() {
    m_queued = false;
    if (!eligible() || m_pending || m_finished || (!m_playing && ready())) return;
    if (!decoderSlots.tryAcquire()) { requestFrame(); return; }
    m_decoder = std::make_shared<JxlDecodeState>();
    m_due = 0; m_clock.invalidate(); decode();
}
void JxlImage::decode() {
    m_pending = true;
    const auto generation = m_generation;
    const auto decoder = m_decoder;
    const auto source = m_source;
    auto *watcher = new QFutureWatcher<FrameResult>(this);
    connect(watcher, &QFutureWatcher<FrameResult>::finished, this, [this, watcher, generation] {
        const auto frame = watcher->result(); watcher->deleteLater();
        if (generation != m_generation) return;
        m_pending = false;
        if (!frame.error.isEmpty()) { m_error = frame.error; m_decoder.reset(); emit errorChanged(); return; }
        m_image = frame.image; ++m_frame; emit frameChanged(); emit readyChanged(); update();
        if (!m_clock.isValid()) m_clock.start();
        m_due += static_cast<long double>(frame.ticks) * frame.denominator * 1000.0L / frame.numerator;
        const auto delay = static_cast<int>(std::clamp(m_due - m_clock.elapsed(), 0.0L, 120000.0L));
        if (frame.last) {
            ++m_cycles; m_decoder.reset();
            if (frame.loops != 0 && m_cycles >= frame.loops) m_finished = true;
        }
        if (!m_playing) { m_decoder.reset(); return; }
        if (!m_finished) m_timer.start(delay);
    });
    watcher->setFuture(QtConcurrent::run([decoder, source]() -> FrameResult {
        FrameResult result;
        try {
            if (source.startsWith("data:image/png;base64,")) {
                if (source.size() >= 900000) throw std::runtime_error("Cached PNG exceeds size limit");
                auto bytes = QByteArray::fromBase64(source.mid(22).toLatin1(), QByteArray::AbortOnBase64DecodingErrors);
                QBuffer input(&bytes); input.open(QIODevice::ReadOnly);
                QImageReader reader(&input, "png");
                const auto size = reader.size();
                if (size.isEmpty() || size.width() > 1024 || size.height() > 1024) throw std::runtime_error("Invalid cached PNG dimensions");
                result.image = reader.read().scaled(256, 256, Qt::KeepAspectRatio, Qt::SmoothTransformation);
                if (result.image.isNull()) throw std::runtime_error("Cached PNG decode failed");
                return result;
            }
            if (!decoder->image) {
                const auto bytes = source.toUtf8();
                decoder->image.emplace(cords::open_image(rust::Str(bytes.constData(), static_cast<size_t>(bytes.size()))));
            }
            const auto frame = (*decoder->image)->next_frame();
            const auto format = frame.bits == 16 ? QImage::Format_RGBA64 : QImage::Format_RGBA8888;
            const QImage canvas(frame.pixels.data(), static_cast<int>(frame.width), static_cast<int>(frame.height), format);
            // The Qt texture is bounded to 256x256. Its copy outlives the Rust
            // frame, whose full-resolution buffer is released immediately.
            result.image = canvas.scaled(256, 256, Qt::KeepAspectRatio, Qt::SmoothTransformation);
            if (result.image.constBits() == frame.pixels.data()) result.image = result.image.copy();
            result.ticks = frame.duration_ticks; result.numerator = frame.ticks_numerator;
            result.denominator = frame.ticks_denominator; result.loops = frame.loops; result.last = frame.last;
        } catch (const std::exception &error) { result.error = QString::fromUtf8(error.what()); }
        return result;
    }));
}
void JxlImage::paint(QPainter *painter) {
    if (m_image.isNull()) return;
    QPainterPath mask;
    if (m_square) mask.addRoundedRect(boundingRect(), 5, 5); else mask.addEllipse(boundingRect());
    painter->setClipPath(mask);
    const auto zoom = std::clamp(m_zoom, 1.0, 6.0);
    const auto side = std::min(m_image.width(), m_image.height()) / zoom;
    const auto x = std::clamp(m_centerX * m_image.width() - side / 2, 0.0, m_image.width() - side);
    const auto y = std::clamp(m_centerY * m_image.height() - side / 2, 0.0, m_image.height() - side);
    painter->drawImage(boundingRect(), m_image, QRectF(x, y, side, side));
}
