#include <QDir>
#include <QFont>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QUrl>

#include "filesystem.h"
#include "foldermodel.h"
#include "jobmanager.h"
#include "opener.h"
#include "thumbnailer.h"

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("com.github.RootGPT_YouTube"));
    app.setApplicationName(QStringLiteral("rootree"));
    app.setDesktopFileName(QStringLiteral("harbour-rootree"));

    // il font di RooTundra (Rajdhani, OFL) per tutta l'app
    const QString fonts = QCoreApplication::applicationDirPath() + QStringLiteral("/qml/fonts/");
    QFontDatabase::addApplicationFont(fonts + QStringLiteral("Rajdhani-Light.ttf"));
    const int id = QFontDatabase::addApplicationFont(fonts + QStringLiteral("Rajdhani-Medium.ttf"));
    if (id >= 0) {
        QFont font(QFontDatabase::applicationFontFamilies(id).value(0));
        font.setWeight(QFont::Medium);
        app.setFont(font);
    }

    qmlRegisterType<FolderModel>("RooTree", 1, 0, "FolderModel");
    FileSystem fs;
    JobManager jobs;
    qmlRegisterSingletonInstance("RooTree", 1, 0, "Fs", &fs);
    qmlRegisterSingletonInstance("RooTree", 1, 0, "Jobs", &jobs);
    Opener opener;
    qmlRegisterSingletonInstance("RooTree", 1, 0, "Opener", &opener);

    QQmlApplicationEngine engine;
    engine.addImageProvider(QStringLiteral("thumb"), new ThumbProvider(false));
    engine.addImageProvider(QStringLiteral("picture"), new ThumbProvider(true));
    // come «dolphin <cartella>»: si apre lì, altrimenti nella home
    const QStringList args = app.arguments();
    const QString start = args.size() > 1 && QDir(args.at(1)).exists() ? QDir(args.at(1)).absolutePath()
                                                                        : QDir::homePath();
    engine.rootContext()->setContextProperty(QStringLiteral("startPath"), start);
    // il QML sta in qml/ accanto al binario vero (vedi CMakeLists.txt): non
    // accanto ad argv[0], che il launcher forgia = /usr/bin/harbour-rootree
    const QString qml = QCoreApplication::applicationDirPath() + QStringLiteral("/qml/Main.qml");
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed,
                     &app, [] { QCoreApplication::exit(255); }, Qt::QueuedConnection);
    engine.load(QUrl::fromLocalFile(qml));
    return app.exec();
}
