#include "filesystem.h"

#include <QJsonDocument>

#include <QDir>
#include <QHash>
#include <QFileInfo>
#include <QFontDatabase>
#include <QProcess>
#include <QCoreApplication>
#include <QCryptographicHash>
#include <QDateTime>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QPointer>
#include <thread>

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

QVariantMap FileSystem::storageInfo(const QString &path) const
{
    const rust::String j = rootree::storage_info(qtrust::str(path));
    return QJsonDocument::fromJson(QByteArray(j.data(), qsizetype(j.size()))).toVariant().toMap();
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

QString FileSystem::makeFile(const QString &parent, const QString &name) const
{
    try {
        rootree::make_file(qtrust::str(parent), qtrust::str(name));
        return {};
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
}

QString FileSystem::trashHint(const QStringList &paths) const
{
    return qtrust::qs(rootree::trash_hint(qtrust::vec(paths)));
}

void FileSystem::trashSetDays(int days)
{
    rootree::trash_set_days(uint32_t(qMax(0, days)));
}

void FileSystem::trashPurge()
{
    rootree::trash_purge_local();
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
    // i server (/@remote/<id>/…) non stanno sul disco del telefono: lo chiede
    // il motore, che li sa raggiungere (16/11: prima rispondeva sempre «no» e
    // il selettore della sincronizzazione ripiegava sulla home del telefono)
    if (path.startsWith(QStringLiteral("/@remote/"))) {
        try {
            rootree::stat_path(qtrust::str(path));
            return true;
        } catch (const rust::Error &) {
            return false;
        }
    }
    return QFileInfo::exists(path);
}

bool FileSystem::isWritable(const QString &path) const
{
    // su un server lo decide il server (un rifiuto arriva come errore)
    if (path.startsWith(QStringLiteral("/@remote/")))
        return true;
    // da root si scrive dappertutto (tranne dentro un archivio)
    if (rootActive())
        return QFileInfo::exists(path) && rootree::archive_of(qtrust::str(path)).empty();
    return ::access(QFile::encodeName(path).constData(), W_OK) == 0;
}

bool FileSystem::rootActive() const
{
    return rootree::root_active();
}

bool FileSystem::developerMode() const
{
    return QFileInfo::exists(QStringLiteral("/usr/bin/devel-su"));
}

bool FileSystem::rootHelperInstalled() const
{
    return QFileInfo::exists(QStringLiteral("/usr/libexec/harbour-rootree/rootree-helper"));
}

void FileSystem::rootStart(const QString &password)
{
    if (m_rootStarting || rootActive())
        return;
    m_rootStarting = true;
    emit rootChanged();
    // devel-su e PAM chiedono un momento: su un thread, l'interfaccia non si ferma
    QPointer<FileSystem> self(this);
    std::thread([self, pw = password.toUtf8()]() mutable {
        QString code, message;
        try {
            rootree::root_start(rust::Str(pw.constData(), size_t(pw.size())));
        } catch (const rust::Error &e) {
            message = QString::fromUtf8(e.what());
            const int colon = message.indexOf(QLatin1Char(':'));
            code = colon > 0 && message.left(colon).toUpper() == message.left(colon) ? message.left(colon) : QStringLiteral("ERR");
            if (code != QStringLiteral("ERR"))
                message = message.mid(colon + 1);
        }
        pw.fill('\0'); // la copia della password non resta in memoria
        QMetaObject::invokeMethod(qApp, [self, code, message] {
            if (!self)
                return;
            self->m_rootStarting = false;
            emit self->rootChanged();
            emit self->rootFinished(code, message);
        }, Qt::QueuedConnection);
    }).detach();
}

void FileSystem::rootStop()
{
    rootree::root_stop();
    emit rootChanged();
}

QVariantMap FileSystem::writeText(const QString &path, const QString &text) const
{
    try {
        return {{QStringLiteral("error"), QString()},
                {QStringLiteral("backup"), qtrust::qs(rootree::write_text(qtrust::str(path), qtrust::str(text)))}};
    } catch (const rust::Error &e) {
        return {{QStringLiteral("error"), QString::fromUtf8(e.what())}, {QStringLiteral("backup"), QString()}};
    }
}

QString FileSystem::setOwner(const QString &path, const QString &user, const QString &group) const
{
    try {
        rootree::set_owner(qtrust::str(path), qtrust::str(user), qtrust::str(group));
        return {};
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
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

QString FileSystem::archiveOf(const QString &path) const
{
    return qtrust::qs(rootree::archive_of(qtrust::str(path)));
}

bool FileSystem::isPrivateKey(const QString &path) const
{
    QFile f(path);
    if (QFileInfo(path).size() > 64 * 1024 || !f.open(QIODevice::ReadOnly))
        return false;
    const QByteArray head = f.read(80);
    return head.startsWith("-----BEGIN ") && head.contains("PRIVATE KEY-----");
}

// ~/.ssh/config: le righe «Chiave valore» (o «Chiave=valore»), raggruppate
// per «Host»; i blocchi «Match» si saltano. ssh prende il PRIMO valore
// trovato per ogni chiave, e così qui.
QVariantList FileSystem::sshConfigHosts() const
{
    QFile f(QDir::homePath() + QStringLiteral("/.ssh/config"));
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text))
        return {};
    struct Block { QStringList patterns; QHash<QString, QString> values; };
    QList<Block> blocks;
    bool inMatch = false;
    const auto lines = QString::fromUtf8(f.readAll()).split(QLatin1Char('\n'));
    for (QString line : lines) {
        line = line.trimmed();
        if (line.isEmpty() || line.startsWith(QLatin1Char('#')))
            continue;
        static const QRegularExpression sep(QStringLiteral("^(\\S+?)(?:\\s*=\\s*|\\s+)(.*)$"));
        const auto m = sep.match(line);
        if (!m.hasMatch())
            continue;
        const QString key = m.captured(1).toLower();
        QString value = m.captured(2).trimmed();
        if (value.size() > 1 && value.startsWith(QLatin1Char('"')) && value.endsWith(QLatin1Char('"')))
            value = value.mid(1, value.size() - 2);
        if (key == QStringLiteral("host")) {
            blocks.append({value.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts), {}});
            inMatch = false;
        } else if (key == QStringLiteral("match")) {
            inMatch = true;
        } else if (!inMatch && !blocks.isEmpty() && !blocks.last().values.contains(key)) {
            blocks.last().values.insert(key, value);
        }
    }
    const QString home = QDir::homePath();
    auto expand = [&](QString v) {
        if (v.startsWith(QStringLiteral("~/")))
            v = home + v.mid(1);
        v.replace(QStringLiteral("%d"), home);
        return v;
    };
    // i ripieghi: i blocchi con caratteri jolly che valgono per tutti («Host *»)
    QHash<QString, QString> fallback;
    for (const Block &b : std::as_const(blocks))
        if (b.patterns.contains(QStringLiteral("*")))
            for (auto it = b.values.cbegin(); it != b.values.cend(); ++it)
                if (!fallback.contains(it.key()))
                    fallback.insert(it.key(), it.value());
    QVariantList out;
    for (const Block &b : std::as_const(blocks)) {
        QStringList names;
        for (const QString &pat : b.patterns)
            if (!pat.contains(QLatin1Char('*')) && !pat.contains(QLatin1Char('?')) && !pat.startsWith(QLatin1Char('!')))
                names << pat;
        if (names.isEmpty())
            continue;
        auto val = [&](const char *k) {
            const QString key = QString::fromLatin1(k);
            return b.values.value(key, fallback.value(key));
        };
        const QString id = val("identityfile");
        out << QVariantMap{
            {QStringLiteral("alias"), names.first()},
            {QStringLiteral("aliases"), names},
            {QStringLiteral("host"), val("hostname").isEmpty() ? names.first() : val("hostname")},
            {QStringLiteral("port"), val("port").isEmpty() ? 22 : val("port").toInt()},
            {QStringLiteral("user"), val("user")},
            {QStringLiteral("identityFile"), id.isEmpty() ? QString() : expand(id)},
        };
    }
    return out;
}

QVariantMap FileSystem::sshKeyInfo(const QString &path) const
{
    QString type, comment;
    // la .pub accanto: «ssh-ed25519 AAAA… commento»
    QFile pub(path + QStringLiteral(".pub"));
    if (pub.open(QIODevice::ReadOnly | QIODevice::Text)) {
        const QStringList f = QString::fromUtf8(pub.readLine(16 * 1024)).trimmed().split(QLatin1Char(' '), Qt::SkipEmptyParts);
        if (!f.isEmpty())
            type = f.at(0);
        if (f.size() > 2)
            comment = f.mid(2).join(QLatin1Char(' '));
    }
    if (type.isEmpty()) {
        // senza .pub: il tipo sta scritto dentro la chiave (anche cifrata, la
        // parte pubblica è in chiaro nel formato OpenSSH); i PEM lo dicono
        // nell'intestazione
        QFile f(path);
        if (f.open(QIODevice::ReadOnly) && f.size() < 64 * 1024) {
            const QByteArray all = f.readAll();
            if (all.contains("BEGIN RSA PRIVATE KEY"))
                type = QStringLiteral("ssh-rsa");
            else if (all.contains("BEGIN EC PRIVATE KEY"))
                type = QStringLiteral("ecdsa");
            else if (all.contains("BEGIN DSA PRIVATE KEY"))
                type = QStringLiteral("ssh-dss");
            else {
                QByteArray b64;
                for (const QByteArray &l : all.split('\n'))
                    if (!l.startsWith("-----"))
                        b64 += l.trimmed();
                const QByteArray raw = QByteArray::fromBase64(b64);
                for (const char *t : {"ssh-ed25519", "ssh-rsa", "ecdsa-sha2-nistp256", "ecdsa-sha2-nistp384",
                                      "ecdsa-sha2-nistp521", "sk-ssh-ed25519@openssh.com", "ssh-dss"})
                    if (raw.contains(t)) { type = QString::fromLatin1(t); break; }
            }
        }
    }
    // in breve: ed25519, rsa, ecdsa…
    QString shortType = type;
    shortType.remove(QStringLiteral("ssh-"));
    if (shortType.startsWith(QStringLiteral("ecdsa")))
        shortType = QStringLiteral("ecdsa");
    if (shortType.startsWith(QStringLiteral("sk-")))
        shortType = QStringLiteral("ed25519 (chiave fisica)");
    if (shortType == QStringLiteral("dss"))
        shortType = QStringLiteral("dsa");
    QStringList hosts;
    const QString real = QFileInfo(path).canonicalFilePath();
    for (const QVariant &v : sshConfigHosts()) {
        const QVariantMap h = v.toMap();
        const QString id = h.value(QStringLiteral("identityFile")).toString();
        if (!id.isEmpty() && (id == path || QFileInfo(id).canonicalFilePath() == real))
            hosts << h.value(QStringLiteral("alias")).toString();
    }
    return {{QStringLiteral("type"), shortType}, {QStringLiteral("comment"), comment}, {QStringLiteral("hosts"), hosts}};
}

QStringList FileSystem::sshKeys() const
{
    QStringList out;
    const QDir dir(QDir::homePath() + QStringLiteral("/.ssh"));
    const auto files = dir.entryInfoList(QDir::Files | QDir::Hidden, QDir::Name);
    for (const QFileInfo &f : files)
        if (isPrivateKey(f.absoluteFilePath()))
            out << f.absoluteFilePath();
    return out;
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
        for (const QString &kind : {QStringLiteral("/archivi"), QStringLiteral("/remoti")}) {
            const QString b = QStandardPaths::writableLocation(QStandardPaths::CacheLocation) + kind;
            const auto dirs = QDir(b).entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot);
            for (const QFileInfo &d : dirs)
                if (d.lastModified() < limit)
                    QDir(d.absoluteFilePath()).removeRecursively();
        }
    }
    return base;
}

QString FileSystem::remoteCacheDir(const QString &path)
{
    // accanto alle voci aperte dagli archivi: stessa pulizia (più di un giorno)
    const QString dir = archiveCache().section(QLatin1Char('/'), 0, -2) + QStringLiteral("/remoti/")
        + QString::fromLatin1(QCryptographicHash::hash(path.toUtf8(), QCryptographicHash::Sha1).toHex().left(16));
    QDir(dir).removeRecursively(); // sempre la versione di adesso
    QDir().mkpath(dir);
    return dir;
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
