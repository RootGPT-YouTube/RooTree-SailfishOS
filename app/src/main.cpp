#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QUrl>

#include "foldermodel.h"

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("com.github.RootGPT_YouTube"));
    app.setApplicationName(QStringLiteral("rootree"));
    app.setDesktopFileName(QStringLiteral("harbour-rootree"));

    qmlRegisterType<FolderModel>("RooTree", 1, 0, "FolderModel");

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("homePath"), QDir::homePath());
    // il QML sta in qml/ accanto al binario (vedi CMakeLists.txt)
    const QString qml = QFileInfo(QString::fromLocal8Bit(argv[0])).absolutePath()
                        + QStringLiteral("/qml/Main.qml");
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed,
                     &app, [] { QCoreApplication::exit(255); }, Qt::QueuedConnection);
    engine.load(QUrl::fromLocalFile(qml));
    return app.exec();
}
