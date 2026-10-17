#include "jxlimage.h"
#include "cords-qt-bridge/src/lib.rs.h"
#include <QCoreApplication>
#include <QCryptographicHash>
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
// Two bounded 128 MiB sandbox heaps. Entries and the FIFO are bounded by
// shared still images and independent active avatar timelines.
QSemaphore decoderSlots(2);
QList<std::weak_ptr<JxlPlayback>> entries, waiting;
QTimer *scheduler = nullptr;
struct FrameResult {
    QImage image;
    QString error;
    quint32 ticks = 0, numerator = 1, denominator = 1, loops = 1;
    bool last = true;
};
struct JxlDecodeState {
    std::optional<rust::Box<cords::NativeImage>> image;
    ~JxlDecodeState() { image.reset(); decoderSlots.release(); }
};
}

// GUI-thread ownership; jobs retain only their sandbox, never a view or this
// object. Weak registry/FIFO entries cannot keep an account's images alive.
class JxlPlayback : public QObject, public std::enable_shared_from_this<JxlPlayback> {
public:
    QString key, source, error;
    QList<QPointer<JxlImage>> views;
    QImage image;
    int serial = 0, generation = 0;
    quint32 cycles = 0;
    bool pending = false, queued = false, finished = false;
    bool animate = false, animated = false;
    QTimer timer, visibility;
    QElapsedTimer clock;
    long double due = 0;
    std::shared_ptr<JxlDecodeState> decoder;

    JxlPlayback(const QString &identity, const QString &data) : key(identity), source(data) {
        timer.setSingleShot(true);
        connect(&timer, &QTimer::timeout, this, [this] { refresh(); });
        visibility.setInterval(250);
        connect(&visibility, &QTimer::timeout, this, [this] { refresh(); });
        visibility.start();
    }
    bool needed() const {
        for (const auto &view : views)
            if (view && view->eligible() && (!view->ready() || (animate && view->playing() && !finished))) return true;
        return false;
    }
    void distribute() {
        // Signal handlers may destroy/rebind delegates. Iterate a stable list
        // and check membership before touching each view.
        const auto targets = views;
        for (const auto &view : targets) {
            if (!view || view->m_playback.get() != this || !view->eligible()) continue;
            if (!error.isEmpty() && view->m_error != error) {
                view->m_error = error; emit view->errorChanged();
            }
            if (!view || view->m_playback.get() != this) continue;
            if (!image.isNull() && (!animate || !view->ready() || view->playing()) && view->m_frame != serial) {
                view->m_image = image; view->m_frame = serial;
                if (view->m_animated != animated) { view->m_animated = animated; emit view->animatedChanged(); }
                if (!view || view->m_playback.get() != this) continue;
                view->update(); emit view->frameChanged();
                if (view && view->m_playback.get() == this) emit view->readyChanged();
            }
        }
    }
    void suspend() {
        if (pending || decoder || queued || timer.isActive()) {
            ++generation; timer.stop(); decoder.reset(); pending = false; queued = false;
            waiting.removeIf([this](const auto &entry) { const auto value = entry.lock(); return !value || value.get() == this; });
        }
    }
    void refresh() {
        views.removeIf([](const auto &view) { return view.isNull(); });
        distribute();
        if (!needed() || !error.isEmpty()) { suspend(); return; }
        if (timer.isActive() || pending || queued) return;
        if (decoder) { decode(); return; }
        queued = true; waiting.append(weak_from_this());
    }
    void pump() {
        queued = false;
        if (!needed() || !error.isEmpty()) return;
        if (!decoderSlots.tryAcquire()) { refresh(); return; }
        decoder = std::make_shared<JxlDecodeState>();
        due = 0; clock.invalidate(); decode();
    }
    void decode() {
        pending = true;
        const auto revision = generation;
        const auto sandbox = decoder;
        const auto data = source;
        const auto weak = weak_from_this();
        auto *watcher = new QFutureWatcher<FrameResult>(this);
        connect(watcher, &QFutureWatcher<FrameResult>::finished, this, [weak, watcher, revision] {
            const auto self = weak.lock();
            const auto frame = watcher->result(); watcher->deleteLater();
            if (!self || revision != self->generation) return;
            self->pending = false;
            if (!frame.error.isEmpty()) {
                self->error = frame.error; self->decoder.reset(); self->distribute(); return;
            }
            self->animated = self->animated || !frame.last || frame.ticks != 0;
            self->image = frame.image; ++self->serial;
            self->distribute();
            if (!self->clock.isValid()) self->clock.start();
            self->due += static_cast<long double>(frame.ticks) * frame.denominator * 1000.0L / frame.numerator;
            const auto delay = static_cast<int>(std::clamp(self->due - self->clock.elapsed(), 0.0L, 120000.0L));
            if (frame.last) {
                ++self->cycles; self->decoder.reset();
                if (frame.loops != 0 && self->cycles >= frame.loops) self->finished = true;
            }
            if (!self->needed()) { self->suspend(); return; }
            self->timer.start(delay);
        });
        watcher->setFuture(QtConcurrent::run([decoder = sandbox, source = data]() -> FrameResult {
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
};

JxlImage::JxlImage(QQuickItem *parent) : QQuickPaintedItem(parent) {
    setAntialiasing(true);
    connect(this, &JxlImage::cropChanged, this, [this] { update(); });
    connect(this, &QQuickItem::visibleChanged, this, &JxlImage::requestFrame);
    const auto observeWindow = [this](QQuickWindow *window) {
        if (window) {
            connect(window, &QWindow::visibilityChanged, this, &JxlImage::requestFrame);
            connect(window, &QWindow::visibleChanged, this, &JxlImage::requestFrame);
        }
        requestFrame();
    };
    connect(this, &QQuickItem::windowChanged, this, observeWindow);
    observeWindow(window());
    m_visibility.setInterval(250);
    connect(&m_visibility, &QTimer::timeout, this, &JxlImage::requestFrame);
    m_visibility.start();
    if (!scheduler) {
        scheduler = new QTimer(QCoreApplication::instance());
        scheduler->setInterval(30);
        QObject::connect(scheduler, &QTimer::timeout, QCoreApplication::instance(), [] {
            while (!waiting.isEmpty()) {
                auto playback = waiting.first().lock();
                if (!playback) { waiting.removeFirst(); continue; }
                if (decoderSlots.available() == 0) break;
                waiting.removeFirst(); playback->pump();
            }
        });
        scheduler->start();
    }
}
JxlImage::~JxlImage() { detach(); }
bool JxlImage::eligible() const {
    if (!isVisible() || !window() || !window()->isVisible() || window()->visibility() == QWindow::Minimized
        || width() <= 0 || height() <= 0 || m_source.isEmpty()) return false;
    for (auto *parent = parentItem(); parent; parent = parent->parentItem()) {
        if (parent->clip() && !parent->boundingRect().intersects(mapRectToItem(parent, boundingRect()))) return false;
    }
    return QRectF(0, 0, window()->width(), window()->height()).intersects(mapRectToScene(boundingRect()));
}
void JxlImage::detach() {
    auto playback = std::move(m_playback);
    if (playback) { playback->views.removeAll(this); playback->refresh(); }
    m_visibility.start();
}
void JxlImage::reset() {
    detach(); m_stillPlayback.reset(); m_animated = false; emit animatedChanged(); m_image = {}; m_error.clear(); m_frame = 0;
    emit readyChanged(); emit errorChanged(); emit frameChanged(); update();
}
void JxlImage::setSource(const QString &source) {
    if (m_source == source) return;
    reset(); m_source = source; emit sourceChanged(); requestFrame();
}
void JxlImage::setAccount(const QString &account) {
    if (m_account == account) return;
    reset(); m_account = account; emit accountChanged(); requestFrame();
}
void JxlImage::setPlaying(bool value) {
    if (m_playing == value) return;
    m_playing = value; emit playingChanged(); requestFrame();
}
void JxlImage::setSquare(bool value) { if (m_square != value) { m_square = value; emit squareChanged(); update(); } }
void JxlImage::requestFrame() {
    if (!eligible()) {
        if (m_playback && m_playback != m_stillPlayback) {
            detach(); m_frame = 0;
            if (m_stillPlayback) { m_image = m_stillPlayback->image; update(); emit frameChanged(); }
        }
        return;
    }
    if (!m_stillPlayback && m_error.isEmpty()) {
        entries.removeIf([](const auto &entry) { return entry.expired(); });
        const auto key = m_account + ':' + QString::fromLatin1(QCryptographicHash::hash(m_source.toUtf8(), QCryptographicHash::Sha256).toHex());
        for (const auto &entry : entries) {
            auto playback = entry.lock();
            if (playback && !playback->animate && playback->key == key) { m_stillPlayback = playback; break; }
        }
        if (!m_stillPlayback) {
            if (entries.size() >= 64) {
                m_error = tr("Visible media limit reached"); emit errorChanged(); return;
            }
            m_stillPlayback = std::make_shared<JxlPlayback>(key, m_source);
            entries.append(m_stillPlayback);
        }
    }
    if (!m_stillPlayback) return;
    const bool active = m_playing && m_stillPlayback->animated && !m_stillPlayback->image.isNull();
    if (!m_playback || m_playback->animate != active) {
        detach();
        if (!active) m_frame = 0;
        if (active) {
            m_playback = std::make_shared<JxlPlayback>(m_stillPlayback->key, m_source);
            m_playback->animate = true;
        } else m_playback = m_stillPlayback;
        m_playback->views.append(this);
    }
    const auto playback = m_playback;
    playback->refresh();
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
