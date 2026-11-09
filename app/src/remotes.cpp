#include "remotes.h"

#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonObject>
#include <QJsonDocument>
#include <QSaveFile>
#include <QStandardPaths>
#include <QFileInfo>
#include <QUuid>
#include <QCoreApplication>
#include <QPointer>
#include <thread>

#include "qtrust.h"

namespace {
// ⭐ porta e metodi d'accesso predefiniti per tipo; i cloud con OAuth non
// chiedono server né password (si accede dal browser, al primo collegamento)
QVariantMap type(const char *type, const QString &label, const char *group, const char *icon, int port,
                 const QStringList &auths, bool needsServer, bool oauth)
{
    return {{QStringLiteral("type"), QString::fromLatin1(type)}, {QStringLiteral("label"), label},
            {QStringLiteral("group"), QString::fromLatin1(group)}, {QStringLiteral("icon"), QString::fromLatin1(icon)},
            {QStringLiteral("port"), port}, {QStringLiteral("auths"), auths},
            {QStringLiteral("needsServer"), needsServer}, {QStringLiteral("oauth"), oauth}};
}
}

static QVariantMap soon(QVariantMap t)
{
    t.insert(QStringLiteral("soon"), true);
    return t;
}

Remotes::Remotes(QObject *parent)
    : QObject(parent)
{
    load();
    watch();
    QObject::connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, [this] {
        load();
        watch(); // una sostituzione atomica (rename) toglie il file dalla sorveglianza
        emit changed();
    });
    QObject::connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, [this] {
        if (m_watcher.files().isEmpty() && QFileInfo::exists(file())) {
            load();
            watch();
            emit changed();
        }
    });
    const QString vault = QStandardPaths::writableLocation(QStandardPaths::AppConfigLocation) + QStringLiteral("/cassaforte.json");
    rootree::vault_set_path(qtrust::str(vault));
}

bool Remotes::hasSecret(const QString &id) const
{
    return !secret(id).isEmpty();
}

QString Remotes::secret(const QString &id) const
{
    if (m_secrets.contains(id))
        return m_secrets.value(id);
    return qtrust::qs(rootree::vault_get(qtrust::str(id)));
}

void Remotes::connect(const QString &id)
{
    QVariantMap p = get(id);
    if (p.isEmpty() || m_connecting.contains(id))
        return;
    m_connecting.insert(id);
    if (m_oneTime.contains(id))
        p.insert(QStringLiteral("mfa"), m_oneTime.take(id));
    const QByteArray json = QJsonDocument(QJsonObject::fromVariantMap(p)).toJson(QJsonDocument::Compact);
    QByteArray sec = secret(id).toUtf8();
    QPointer<Remotes> self(this);
    std::thread([self, id, json, sec]() mutable {
        QString code, message, path;
        try {
            path = qtrust::qs(rootree::remote_connect(rust::Str(json.constData(), size_t(json.size())),
                                                      rust::Str(sec.constData(), size_t(sec.size()))));
        } catch (const rust::Error &e) {
            message = QString::fromUtf8(e.what());
            const int colon = message.indexOf(QLatin1Char(':'));
            const QString head = colon > 0 ? message.left(colon) : QString();
            if (!head.isEmpty() && head == head.toUpper() && !head.contains(QLatin1Char(' '))) {
                code = head;
                message = message.mid(colon + 1);
            } else {
                code = QStringLiteral("ERR");
            }
        }
        sec.fill('\0');
        QMetaObject::invokeMethod(qApp, [self, id, code, message, path] {
            if (!self)
                return;
            self->m_connecting.remove(id);
            if (code.isEmpty())
                self->m_entry.insert(id, QStringLiteral("/@remote/") + id + (path == QStringLiteral("/") ? QString() : path));
            emit self->connectFinished(id, code, message, self->m_entry.value(id));
        }, Qt::QueuedConnection);
    }).detach();
}

void Remotes::setStartPath(const QString &id, const QString &serverPath)
{
    QVariantMap p = get(id);
    if (p.isEmpty())
        return;
    p.insert(QStringLiteral("path"), serverPath);
    save(p);
    if (m_entry.contains(id))
        m_entry.insert(id, QStringLiteral("/@remote/") + id + (serverPath == QStringLiteral("/") ? QString() : serverPath));
}

static QPair<QString, QString> splitCode(const QString &what)
{
    const int colon = what.indexOf(QLatin1Char(':'));
    const QString head = colon > 0 ? what.left(colon) : QString();
    if (!head.isEmpty() && head == head.toUpper() && !head.contains(QLatin1Char(' ')))
        return {head, what.mid(colon + 1)};
    return {QStringLiteral("ERR"), what};
}

void Remotes::authorize(const QString &id)
{
    const QVariantMap p = get(id);
    if (p.isEmpty() || m_authorizing.contains(id))
        return;
    m_authorizing.insert(id);
    const QByteArray kind = p.value(QStringLiteral("type")).toString().toUtf8();
    const QByteArray cid = p.value(QStringLiteral("clientId")).toString().toUtf8();
    const QByteArray bid = id.toUtf8();
    QPointer<Remotes> self(this);
    std::thread([self, id, kind, cid, bid] {
        auto s = [](const QByteArray &b) { return rust::Str(b.constData(), size_t(b.size())); };
        QString code, message, token;
        try {
            const QString url = qtrust::qs(rootree::oauth_begin(s(bid), s(kind), s(cid)));
            QMetaObject::invokeMethod(qApp, [self, id, url] { if (self) emit self->authUrl(id, url); }, Qt::QueuedConnection);
            // cinque minuti per accedere nel browser
            token = qtrust::qs(rootree::oauth_complete(s(bid), s(kind), s(cid), 300));
        } catch (const rust::Error &e) {
            const auto cm = splitCode(QString::fromUtf8(e.what()));
            code = cm.first;
            message = cm.second;
        }
        QMetaObject::invokeMethod(qApp, [self, id, code, message, token] {
            if (!self)
                return;
            self->m_authorizing.remove(id);
            if (code.isEmpty())
                self->m_secrets.insert(id, token);
            emit self->authFinished(id, code, message);
        }, Qt::QueuedConnection);
    }).detach();
}

void Remotes::megaRevoke(const QString &id)
{
    const QByteArray bid = id.toUtf8();
    QPointer<Remotes> self(this);
    std::thread([self, id, bid] {
        QString error;
        try {
            rootree::mega_revoke(rust::Str(bid.constData(), size_t(bid.size())));
        } catch (const rust::Error &e) {
            error = QString::fromUtf8(e.what());
        }
        QMetaObject::invokeMethod(qApp, [self, id, error] { if (self) emit self->megaRevoked(id, error); }, Qt::QueuedConnection);
    }).detach();
}

void Remotes::trash(const QString &id, const QString &action, const QString &item)
{
    const QByteArray bid = id.toUtf8(), bact = action.toUtf8(), bitem = item.toUtf8();
    QPointer<Remotes> self(this);
    std::thread([self, id, action, bid, bact, bitem] {
        QString error, json;
        try {
            json = qtrust::qs(rootree::remote_trash(rust::Str(bid.constData(), size_t(bid.size())),
                                                    rust::Str(bact.constData(), size_t(bact.size())),
                                                    rust::Str(bitem.constData(), size_t(bitem.size()))));
        } catch (const rust::Error &e) {
            error = QString::fromUtf8(e.what());
        }
        QMetaObject::invokeMethod(qApp, [self, id, action, error, json] {
            if (self)
                emit self->trashDone(id, action, error, json);
        }, Qt::QueuedConnection);
    }).detach();
}

void Remotes::cancelAuth(const QString &id)
{
    rootree::oauth_cancel(qtrust::str(id));
}

void Remotes::forgetSecret(const QString &id)
{
    m_secrets.remove(id);
    try { rootree::vault_remove(qtrust::str(id)); } catch (const rust::Error &) {}
    disconnect(id);
}

QString Remotes::trust(const QString &id)
{
    try {
        rootree::remote_trust(qtrust::str(id));
        return {};
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
}

void Remotes::disconnect(const QString &id)
{
    rootree::remote_disconnect(qtrust::str(id));
    m_entry.remove(id);
    emit changed();
}

bool Remotes::isConnected(const QString &id) const
{
    return rootree::remote_connected(qtrust::str(id));
}

bool Remotes::vaultExists() const
{
    return rootree::vault_exists();
}

bool Remotes::vaultUnlocked() const
{
    return rootree::vault_unlocked();
}

QString Remotes::vaultCreate(const QString &master)
{
    try {
        rootree::vault_create(qtrust::str(master));
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
    emit vaultChanged();
    return {};
}

QString Remotes::vaultUnlock(const QString &master)
{
    try {
        rootree::vault_unlock(qtrust::str(master));
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
    emit vaultChanged();
    return {};
}

void Remotes::vaultLock()
{
    rootree::vault_lock();
    emit vaultChanged();
}

QString Remotes::remember(const QString &id)
{
    if (!m_secrets.contains(id))
        return {};
    try {
        rootree::vault_set(qtrust::str(id), qtrust::str(m_secrets.value(id)));
        return {};
    } catch (const rust::Error &e) {
        return QString::fromUtf8(e.what());
    }
}

QVariantList Remotes::types() const
{
    const QStringList pw{QStringLiteral("password")};
    return {
        type("sftp", QStringLiteral("SFTP (SSH)"), "server", "ssh", 22, {QStringLiteral("password"), QStringLiteral("key")}, true, false),
        type("ftp", QStringLiteral("FTP / FTPS"), "server", "server", 21, {QStringLiteral("password"), QStringLiteral("anonymous")}, true, false),
        type("webdav", QStringLiteral("WebDAV"), "server", "network", 443, pw, true, false),
        type("nextcloud", QStringLiteral("Nextcloud / ownCloud"), "server", "nextcloud", 443, pw, true, false),
        type("smb", QStringLiteral("SMB (Windows, Samba)"), "server", "smb", 445, {QStringLiteral("password"), QStringLiteral("guest")}, true, false),
        type("s3", QStringLiteral("S3 / compatibile"), "server", "s3", 443, pw, true, false),
        type("dropbox", QStringLiteral("Dropbox"), "cloud", "dropbox", 0, {QStringLiteral("oauth")}, false, true),
        // un cloud alla volta (scelta dell'utente, 08/11): questi arrivano dopo Dropbox
        type("mega", QStringLiteral("Mega"), "cloud", "cloud", 0, pw, false, false),
    };
}

QVariantMap Remotes::typeInfo(const QString &t) const
{
    for (const QVariant &v : types())
        if (v.toMap().value(QStringLiteral("type")).toString() == t)
            return v.toMap();
    return {};
}

QString Remotes::file() const
{
    return QStandardPaths::writableLocation(QStandardPaths::AppConfigLocation) + QStringLiteral("/connessioni.json");
}

void Remotes::watch()
{
    const QString f = file();
    QDir().mkpath(QFileInfo(f).absolutePath());
    if (!m_watcher.directories().contains(QFileInfo(f).absolutePath()))
        m_watcher.addPath(QFileInfo(f).absolutePath());
    if (QFileInfo::exists(f) && !m_watcher.files().contains(f))
        m_watcher.addPath(f);
}

void Remotes::load()
{
    QFile f(file());
    if (f.open(QIODevice::ReadOnly))
        m_list = QJsonDocument::fromJson(f.readAll()).array().toVariantList();
}

void Remotes::store()
{
    QDir().mkpath(QFileInfo(file()).absolutePath());
    QSaveFile f(file());
    // solo l'utente: ci sono nomi di server e di utenti
    if (f.open(QIODevice::WriteOnly)) {
        f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        f.write(QJsonDocument(QJsonArray::fromVariantList(m_list)).toJson());
        f.commit();
    }
    watch();
    emit changed();
}

QVariantMap Remotes::get(const QString &id) const
{
    for (const QVariant &v : m_list)
        if (v.toMap().value(QStringLiteral("id")).toString() == id)
            return v.toMap();
    return {};
}

QString Remotes::save(const QVariantMap &profile)
{
    QVariantMap p = profile;
    // le password non vanno mai nel file
    const QString secret = p.take(QStringLiteral("secret")).toString();
    QString id = p.value(QStringLiteral("id")).toString();
    if (id.isEmpty()) {
        id = QUuid::createUuid().toString(QUuid::WithoutBraces);
        p.insert(QStringLiteral("id"), id);
        m_list.append(p);
    } else {
        const QVariantMap old = get(id);
        for (QVariant &v : m_list)
            if (v.toMap().value(QStringLiteral("id")).toString() == id)
                v = p;
        // ⛔ una connessione aperta non deve restare coi dati vecchi (08/11:
        //    la cartella iniziale cambiata da «Modifica» veniva ignorata,
        //    si rientrava dalla cartella d'ingresso del collegamento)
        static const char *const how[] = {"type", "host", "port", "user", "auth", "keyPath", "secure", "share", "domain", "bucket", "region"};
        bool reconnect = false;
        for (const char *k : how)
            reconnect |= old.value(QLatin1String(k)) != p.value(QLatin1String(k));
        if (reconnect) {
            if (isConnected(id))
                rootree::remote_disconnect(qtrust::str(id));
            m_entry.remove(id);
        } else if (m_entry.contains(id)) {
            const QString path = p.value(QStringLiteral("path")).toString().trimmed();
            m_entry.insert(id, QStringLiteral("/@remote/") + id + (path.isEmpty() || path == QStringLiteral("/") ? QString() : path));
            // vuota = la home sul server, che si sa solo collegandosi: si ricollega
            if (path.isEmpty()) {
                rootree::remote_disconnect(qtrust::str(id));
                m_entry.remove(id);
            }
        }
    }
    if (!secret.isEmpty())
        m_secrets.insert(id, secret);
    store();
    return id;
}

void Remotes::remove(const QString &id)
{
    if (get(id).value(QStringLiteral("mfaTrusted")).toBool())
        megaRevoke(id);
    for (int i = m_list.size() - 1; i >= 0; --i)
        if (m_list.at(i).toMap().value(QStringLiteral("id")).toString() == id)
            m_list.removeAt(i);
    m_secrets.remove(id);
    try { rootree::vault_remove(qtrust::str(id)); } catch (const rust::Error &) {}
    disconnect(id);
    store();
}

QString Remotes::duplicate(const QString &id)
{
    QVariantMap p = get(id);
    if (p.isEmpty())
        return {};
    p.remove(QStringLiteral("id"));
    p.insert(QStringLiteral("name"), p.value(QStringLiteral("name")).toString() + QStringLiteral(" (2)"));
    return save(p);
}

QString Remotes::describe(const QVariantMap &p) const
{
    const QVariantMap t = typeInfo(p.value(QStringLiteral("type")).toString());
    if (!t.value(QStringLiteral("needsServer")).toBool())
        return p.value(QStringLiteral("user")).toString().isEmpty() ? t.value(QStringLiteral("label")).toString()
                                                                    : p.value(QStringLiteral("user")).toString();
    QString s;
    const QString user = p.value(QStringLiteral("user")).toString();
    if (!user.isEmpty() && p.value(QStringLiteral("auth")).toString() != QStringLiteral("anonymous"))
        s += user + QLatin1Char('@');
    s += p.value(QStringLiteral("host")).toString();
    const int port = p.value(QStringLiteral("port")).toInt();
    if (port > 0 && port != t.value(QStringLiteral("port")).toInt())
        s += QLatin1Char(':') + QString::number(port);
    const bool smb = p.value(QStringLiteral("type")).toString() == QStringLiteral("smb");
    const QString path = p.value(smb ? QStringLiteral("share") : QStringLiteral("path")).toString();
    if (!path.isEmpty())
        s += (path.startsWith(QLatin1Char('/')) ? QString() : QStringLiteral("/")) + path;
    return s;
}
