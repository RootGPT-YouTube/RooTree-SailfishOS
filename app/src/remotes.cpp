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

Remotes::Remotes(QObject *parent)
    : QObject(parent)
{
    load();
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
    const QVariantMap p = get(id);
    if (p.isEmpty() || m_connecting.contains(id))
        return;
    m_connecting.insert(id);
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
        type("nextcloud", QStringLiteral("Nextcloud / ownCloud"), "server", "cloud", 443, pw, true, false),
        type("smb", QStringLiteral("SMB (Windows, Samba)"), "server", "smb", 445, {QStringLiteral("password"), QStringLiteral("guest")}, true, false),
        type("s3", QStringLiteral("S3 / compatibile"), "server", "s3", 443, pw, true, false),
        type("gdrive", QStringLiteral("Google Drive"), "cloud", "gdrive", 0, {QStringLiteral("oauth")}, false, true),
        type("onedrive", QStringLiteral("OneDrive"), "cloud", "onedrive", 0, {QStringLiteral("oauth")}, false, true),
        type("dropbox", QStringLiteral("Dropbox"), "cloud", "dropbox", 0, {QStringLiteral("oauth")}, false, true),
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
        for (QVariant &v : m_list)
            if (v.toMap().value(QStringLiteral("id")).toString() == id)
                v = p;
    }
    if (!secret.isEmpty())
        m_secrets.insert(id, secret);
    store();
    return id;
}

void Remotes::remove(const QString &id)
{
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
