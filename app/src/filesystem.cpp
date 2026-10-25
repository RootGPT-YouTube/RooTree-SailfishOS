#include "filesystem.h"

#include <QDir>
#include <QFileInfo>

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
