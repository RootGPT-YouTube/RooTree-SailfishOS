#include "filesystem.h"

#include <QDir>
#include <QFileInfo>
#include <QFontDatabase>
#include <QProcess>
#include <QCryptographicHash>
#include <QDateTime>
#include <QRegularExpression>
#include <QStandardPaths>

#include <unistd.h>

#include "qtrust.h"

FileSystem::FileSystem(QObject *parent)
    : QObject(parent)
{
    m_bookmarks = m_settings.value(QStringLiteral("bookmarks")).toStringList();
}

QString FileSystem::home() const
{
    return QDir::homePath();
}

QVariantList FileSystem::places() const
{
    QVariantList out;
    for (const auto &p : rootree::places(qtrust::str(home()))) {
        out << QVariantMap{
            {QStringLiteral("kind"), int(p.kind)},
            {QStringLiteral("label"), qtrust::qs(p.label)},
            {QStringLiteral("path"), qtrust::qs(p.path)},
            {QStringLiteral("total"), qint64(p.total)},
            {QStringLiteral("free"), qint64(p.free)},
        };
    }
    return out;
}

QVariantMap FileSystem::info(const QString &path) const
{
    try {
        const auto e = rootree::stat_path(qtrust::str(path));
        return {
            {QStringLiteral("name"), qtrust::qs(e.name)},
            {QStringLiteral("path"), qtrust::qs(e.path)},
            {QStringLiteral("isDir"), e.kind == 1},
            {QStringLiteral("kind"), int(e.kind)},
            {QStringLiteral("isLink"), e.is_link},
            {QStringLiteral("linkBroken"), e.link_broken},
            {QStringLiteral("linkTarget"), qtrust::qs(e.link_target)},
            {QStringLiteral("size"), qint64(e.size)},
            {QStringLiteral("mtime"), qint64(e.mtime)},
            {QStringLiteral("mode"), int(e.mode)},
            {QStringLiteral("uid"), int(e.uid)},
            {QStringLiteral("gid"), int(e.gid)},
            {QStringLiteral("user"), qtrust::qs(rootree::user_name(e.uid))},
            {QStringLiteral("group"), qtrust::qs(rootree::group_name(e.gid))},
            {QStringLiteral("category"), qtrust::qs(e.category)},
            // senza root si cambiano i permessi solo di ciò che è nostro
            {QStringLiteral("mine"), e.uid == ::getuid()},
        };
    } catch (const rust::Error &e) {
        return {{QStringLiteral("error"), QString::fromUtf8(e.what())}};
    }
}

QString FileSystem::makeDir(const QString &parent, const QString &name) const
{
    try {
        rootree::make_dir(qtrust::str(parent), qtrust::str(name));
        return {};
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
}

QString FileSystem::rename(const QString &path, const QString &newName) const
{
    try {
        rootree::rename_entry(qtrust::str(path), qtrust::str(newName));
        return {};
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
}

QString FileSystem::setMode(const QString &path, int mode) const
{
    try {
        rootree::set_mode(qtrust::str(path), uint32_t(mode));
        return {};
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
}

bool FileSystem::exists(const QString &path) const
{
    return QFileInfo::exists(path);
}

bool FileSystem::isWritable(const QString &path) const
{
    return ::access(QFile::encodeName(path).constData(), W_OK) == 0;
}

QString FileSystem::parentOf(const QString &path) const
{
    if (path == QLatin1String("/"))
        return path;
    return QFileInfo(path).absolutePath();
}

QString FileSystem::join(const QString &dir, const QString &name) const
{
    return QDir::cleanPath(dir + QLatin1Char('/') + name);
}

bool FileSystem::isArchive(const QString &name) const
{
    return rootree::is_archive(qtrust::str(name));
}

bool FileSystem::canCreateArchive(const QString &name) const
{
    return rootree::can_create_archive(qtrust::str(name));
}

QString FileSystem::archiveStem(const QString &name) const
{
    return qtrust::qs(rootree::archive_stem(qtrust::str(name)));
}

// la cache delle voci aperte da dentro un archivio: una cartella per voce,
// col nome vero del file dentro (le app lo mostrano); al primo uso della
// sessione si buttano quelle di più di un giorno
static QString archiveCache()
{
    const QString base = QStandardPaths::writableLocation(QStandardPaths::CacheLocation) + QStringLiteral("/archivi");
    static bool cleaned = false;
    if (!cleaned) {
        cleaned = true;
        const QDateTime limit = QDateTime::currentDateTime().addDays(-1);
        const auto dirs = QDir(base).entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot);
        for (const QFileInfo &d : dirs)
            if (d.lastModified() < limit)
                QDir(d.absoluteFilePath()).removeRecursively();
    }
    return base;
}

QString FileSystem::extractForOpen(const QString &path)
{
    m_lastError.clear();
    const QString dir = archiveCache() + QLatin1Char('/')
        + QString::fromLatin1(QCryptographicHash::hash(path.toUtf8(), QCryptographicHash::Sha1).toHex().left(16));
    QDir(dir).removeRecursively(); // l'archivio può essere cambiato
    try {
        return qtrust::qs(rootree::archive_extract_one(qtrust::str(path), qtrust::str(dir)));
    } catch (const rust::Error &e) {
        m_lastError = QString::fromUtf8(e.what());
        return {};
    }
}

QVariantList FileSystem::bookmarks() const
{
    QVariantList out;
    for (const QString &p : m_bookmarks)
        out << QVariantMap{{QStringLiteral("path"), p},
                           {QStringLiteral("name"), p == QLatin1String("/") ? p : QFileInfo(p).fileName()},
                           {QStringLiteral("exists"), QFileInfo(p).isDir()}};
    return out;
}

bool FileSystem::isBookmarked(const QString &path) const
{
    return m_bookmarks.contains(path);
}

void FileSystem::addBookmark(const QString &path)
{
    if (m_bookmarks.contains(path))
        return;
    m_bookmarks << path;
    m_settings.setValue(QStringLiteral("bookmarks"), m_bookmarks);
    emit bookmarksChanged();
}

void FileSystem::removeBookmark(const QString &path)
{
    if (m_bookmarks.removeAll(path) == 0)
        return;
    m_settings.setValue(QStringLiteral("bookmarks"), m_bookmarks);
    emit bookmarksChanged();
}

namespace {
// `dconf dump` di una cartella: chiave → valore senza apici
QVariantMap dconfDump(const QString &dir)
{
    QProcess p;
    p.start(QStringLiteral("dconf"), {QStringLiteral("dump"), dir});
    QVariantMap out;
    if (!p.waitForFinished(2000))
        return out;
    QString section;
    const auto lines = QString::fromUtf8(p.readAllStandardOutput()).split(QLatin1Char('\n'));
    for (const QString &line : lines) {
        if (line.startsWith(QLatin1Char('['))) {
            section = line.mid(1, line.size() - 2);
            continue;
        }
        const int eq = line.indexOf(QLatin1Char('='));
        if (eq <= 0)
            continue;
        QString v = line.mid(eq + 1).trimmed();
        if (v.size() >= 2 && v.startsWith(QLatin1Char('\'')) && v.endsWith(QLatin1Char('\'')))
            v = v.mid(1, v.size() - 2);
        const QString key = (section == QLatin1String("/") ? QString() : section + QLatin1Char('/')) + line.left(eq);
        out.insert(key, v);
    }
    return out;
}
}

QVariantMap FileSystem::ambience() const
{
    const QVariantMap theme = dconfDump(QStringLiteral("/desktop/jolla/theme/"));
    QVariantMap out;
    for (const char *k : {"highlight", "primary", "secondary", "secondaryHighlight"}) {
        const QString v = theme.value(QStringLiteral("color/") + QLatin1String(k)).toString();
        if (QRegularExpression(QStringLiteral("^#[0-9a-fA-F]{6,8}$")).match(v).hasMatch())
            out.insert(QLatin1String(k), v);
    }
    // color_scheme 1 = ambience chiara (testo scuro)
    out.insert(QStringLiteral("lightScheme"), theme.value(QStringLiteral("color_scheme")).toString() == QLatin1String("1"));
    static QString font = [] {
        const int id = QFontDatabase::addApplicationFont(QStringLiteral("/usr/share/fonts/sail-sans-pro/SailSansPro-Light.ttf"));
        return id < 0 ? QString() : QFontDatabase::applicationFontFamilies(id).value(0);
    }();
    out.insert(QStringLiteral("font"), font);
    return out;
}
