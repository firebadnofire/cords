#include "controller.h"
#include "platformevents.h"
#include "notifications.h"
#include <QApplication>
#include <QCommandLineParser>
#include <QDir>
#include <QFileInfo>
#include <QIcon>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSettings>
#include <QStandardPaths>
#include <QThread>
#include <iostream>

class ActivityFilter final : public QObject {
public:
    explicit ActivityFilter(Controller &controller) : m_controller(controller) {}
    bool eventFilter(QObject *, QEvent *event) override {
        if (event->type() == QEvent::KeyPress || event->type() == QEvent::MouseButtonPress || event->type() == QEvent::Wheel)
            m_controller.recordActivity();
        return false;
    }
private: Controller &m_controller;
};
QString stateDirectory() {
    if (qEnvironmentVariableIsSet("CORDS_CLIENT_STATE")) return qEnvironmentVariable("CORDS_CLIENT_STATE");
#ifdef Q_OS_WIN
    const auto root = qEnvironmentVariable("APPDATA");
    if (root.isEmpty()) throw std::runtime_error("APPDATA is unavailable; set CORDS_CLIENT_STATE explicitly");
#else
    const auto root = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
#endif
    return QDir(root).filePath("chat.cords.desktop");
}
QString migrationsDirectory() {
    if (qEnvironmentVariableIsSet("CORDS_CLIENT_MIGRATIONS")) return qEnvironmentVariable("CORDS_CLIENT_MIGRATIONS");
    const QDir executable(QCoreApplication::applicationDirPath());
    const auto packaged = executable.absoluteFilePath("../share/cords/migrations/sqlite");
    if (QFileInfo::exists(packaged)) return packaged;
    const auto bundle = executable.absoluteFilePath("../Resources/cords/migrations/sqlite");
    if (QFileInfo::exists(bundle)) return bundle;
#ifdef CORDS_SOURCE_MIGRATIONS
    if (QFileInfo::exists(QStringLiteral(CORDS_SOURCE_MIGRATIONS))) return QStringLiteral(CORDS_SOURCE_MIGRATIONS);
#endif
    throw std::runtime_error("SQLite migrations are missing; repair the package or set CORDS_CLIENT_MIGRATIONS");
}
int main(int argc, char **argv) {
    QApplication app(argc, argv);
    QCoreApplication::setOrganizationName("Cords");
    QCoreApplication::setApplicationName("Cords");
    QCoreApplication::setApplicationVersion("0.1.0");
    QQuickStyle::setStyle("Basic");
    QGuiApplication::setWindowIcon(QIcon(":/128x128.png"));
    QCommandLineParser parser;
    parser.setApplicationDescription("Cords — Right on the wire. Native Qt Quick client.");
    parser.addHelpOption(); parser.addVersionOption();
    parser.addOption({"list-accounts", "Print picker-safe registered account IDs and labels."});
    parser.addOption({"account", "Unlock the exact registered account ID.", "id"});
    parser.addOption({"password-stdin", "Read one UTF-8 password line from redirected stdin."});
    parser.addOption({"capture", "Capture the native window for local layout validation.", "path"});
    parser.addOption({"capture-after", "Delay a local validation capture (0 to 120000 milliseconds).", "milliseconds", "1800"});
    parser.addOption({"quit-after", "Quit cleanly after a bounded number of milliseconds (local acceptance).", "milliseconds"});
    parser.addOption({"window-size", "Set the native window size for local layout acceptance.", "widthxheight"});
    parser.process(app);
    try {
        const auto root = stateDirectory();
        QSettings::setDefaultFormat(QSettings::IniFormat);
        QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, root);
        Controller controller(root, migrationsDirectory(), qEnvironmentVariable("CORDS_CLIENT_CA"));
        if (parser.isSet("list-accounts")) {
            for (int attempt = 0; attempt < 300 && controller.state().isEmpty(); ++attempt) { controller.poll(); QThread::msleep(10); }
            if (controller.state().isEmpty()) throw std::runtime_error("Timed out opening account registry");
            if (!controller.state().value("error").toString().isEmpty()) throw std::runtime_error(controller.state().value("error").toString().toStdString());
            QJsonArray result;
            for (int index = 0; index < controller.accounts()->rowCount(); ++index) {
                const auto row = controller.accounts()->get(index);
                result.append(QJsonObject{{"account_id", row["id"].toString()}, {"label", row["name"].toString()}});
            }
            std::cout << QJsonDocument(result).toJson().constData(); return 0;
        }
        if (parser.isSet("account") != parser.isSet("password-stdin")) throw std::runtime_error("--account and --password-stdin must be used together");
        if (parser.isSet("account")) {
            controller.unlockStdin(parser.value("account"));
        }
        if (parser.isSet("quit-after")) {
            bool valid = false;
            const auto delay = parser.value("quit-after").toInt(&valid);
            if (!valid || delay < 100 || delay > 60000) throw std::runtime_error("--quit-after must be 100–60000 milliseconds");
            QTimer::singleShot(delay, &app, &QCoreApplication::quit);
        }
        ActivityFilter filter(controller); app.installEventFilter(&filter);
        QQmlApplicationEngine engine;
        engine.setInitialProperties({{"desktop", QVariant::fromValue(&controller)}});
        QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app, [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
        engine.loadFromModule("Cords", "Main");
        if (engine.rootObjects().isEmpty()) return 1;
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (!window) throw std::runtime_error("Native application window is missing");
        PlatformEvents platformEvents(controller, *window);
        NativeNotifications notifications(controller, *window);
        if (window && parser.isSet("window-size")) {
            const auto dimensions = parser.value("window-size").split('x');
            bool widthValid = false, heightValid = false;
            const int width = dimensions.value(0).toInt(&widthValid), height = dimensions.value(1).toInt(&heightValid);
            if (dimensions.size() != 2 || !widthValid || !heightValid || width < 680 || height < 520 || width > 7680 || height > 4320)
                throw std::runtime_error("--window-size requires dimensions between 680x520 and 7680x4320");
            window->resize(width, height);
        }
        if (window && parser.isSet("capture")) {
            const auto path = parser.value("capture");
            bool valid = false;
            const int delay = parser.value("capture-after").toInt(&valid);
            if (!valid || delay < 0 || delay > 120000) throw std::runtime_error("--capture-after requires 0 to 120000 milliseconds");
            QTimer::singleShot(delay, window, [window, path] {
                if (!window->grabWindow().save(path)) qCritical("Native window capture failed");
            });
        }
        return app.exec();
    } catch (const std::exception &error) {
        std::cerr << "Cords startup failed: " << error.what() << '\n'; return 1;
    }
}
