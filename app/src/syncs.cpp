#include "syncs.h"

#include "jobmanager.h"
#include "qtrust.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSaveFile>
#include <QStandardPaths>
#include <QUuid>

namespace {
// i cambi arrivano a raffiche (una foto = .part, rinomina, miniatura…):
// si aspetta la calma prima di sincronizzare
constexpr int kQuiet = 10000;
// dopo un giro, i cambi che ha fatto lui stesso non ne chiedono un altro
constexpr int kAfter = 4000;
// inotify ha un limite di cartelle sorvegliate per utente: si resta larghi
constexpr int kMaxDirs = 4000;
const QString kId = QStringLiteral("id");
}

Syncs::Syncs(JobManager *jobs, QObject *parent)
    : QObject(parent)
    , m_jobs(jobs)
{
    load();
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, &Syncs::onDirChanged);
    connect(m_jobs, &JobManager::finished, this, [this](int job, int kind, int, const QString &) {
        if (kind != JobManager::Sync || !m_running.contains(quint64(job)))
            return;
        const QString id = m_running.take(quint64(job));
        // le cartelle nuove scese col giro si sorvegliano anche loro
        watch();
        QTimer::singleShot(kAfter, this, [this, id] {
            if (!running(id))
                m_muted.remove(id);
        });
    });
    watch();
}

QString Syncs::file() const
{
    return QStandardPaths::writableLocation(QStandardPaths::AppConfigLocation) + QStringLiteral("/sincronizzazioni.json");
}

QString Syncs::stateDir() const
{
    return QStandardPaths::writableLocation(QStandardPaths::AppDataLocation) + QStringLiteral("/sync");
}

void Syncs::load()
{
    QFile f(file());
    if (f.open(QIODevice::ReadOnly))
        m_list = QJsonDocument::fromJson(f.readAll()).array().toVariantList();
}

void Syncs::store()
{
    QDir().mkpath(QFileInfo(file()).absolutePath());
    QSaveFile f(file());
    if (f.open(QIODevice::WriteOnly)) {
        f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        f.write(QJsonDocument(QJsonArray::fromVariantList(m_list)).toJson());
        f.commit();
    }
    emit changed();
}

QVariantMap Syncs::get(const QString &id) const
{
    for (const QVariant &v : m_list)
        if (v.toMap().value(kId).toString() == id)
            return v.toMap();
    return {};
}

QString Syncs::save(const QVariantMap &sync)
{
    QVariantMap s = sync;
    QString id = s.value(kId).toString();
    if (id.isEmpty()) {
        id = QUuid::createUuid().toString(QUuid::WithoutBraces).left(8);
        s.insert(kId, id);
        m_list << s;
    } else {
        bool found = false;
        for (QVariant &v : m_list) {
            if (v.toMap().value(kId).toString() == id) {
                const QVariantMap old = v.toMap();
                // cartelle cambiate: lo stato vecchio non vale più (si riparte
                // come la prima volta: uguali se hanno la stessa dimensione)
                if (old.value(QStringLiteral("local")) != s.value(QStringLiteral("local"))
                    || old.value(QStringLiteral("remote")) != s.value(QStringLiteral("remote"))
                    || old.value(QStringLiteral("remotePath")) != s.value(QStringLiteral("remotePath")))
                    rootree::sync_forget(qtrust::str(stateDir()), qtrust::str(id));
                v = s;
                found = true;
            }
        }
        if (!found)
            m_list << s;
    }
    store();
    watch();
    return id;
}

void Syncs::remove(const QString &id)
{
    for (int i = m_list.size() - 1; i >= 0; --i)
        if (m_list.at(i).toMap().value(kId).toString() == id)
            m_list.removeAt(i);
    rootree::sync_forget(qtrust::str(stateDir()), qtrust::str(id));
    store();
    watch();
}

QVariantList Syncs::forRemote(const QString &remoteId) const
{
    QVariantList out;
    for (const QVariant &v : m_list)
        if (v.toMap().value(QStringLiteral("remote")).toString() == remoteId)
            out << v;
    return out;
}

int Syncs::start(const QString &id, bool confirmed)
{
    if (running(id))
        return -1;
    const QVariantMap s = get(id);
    if (s.isEmpty())
        return -1;
    const QByteArray json = QJsonDocument(QJsonObject::fromVariantMap(s)).toJson(QJsonDocument::Compact);
    m_muted.insert(id);
    if (QTimer *t = m_timers.value(id))
        t->stop();
    const quint64 job = rootree::job_sync(qtrust::str(QString::fromUtf8(json)), qtrust::str(stateDir()), confirmed);
    m_running.insert(job, id);
    return m_jobs->track(job);
}

void Syncs::setResult(const QString &id, const QString &result, bool error)
{
    for (QVariant &v : m_list) {
        QVariantMap s = v.toMap();
        if (s.value(kId).toString() != id)
            continue;
        s.insert(QStringLiteral("lastRun"), QDateTime::currentMSecsSinceEpoch());
        s.insert(QStringLiteral("lastResult"), result);
        s.insert(QStringLiteral("lastError"), error);
        v = s;
    }
    store();
}

void Syncs::addTree(const QString &dir, bool recursive, int &budget)
{
    if (budget <= 0)
        return;
    --budget;
    m_watcher.addPath(dir);
    if (!recursive)
        return;
    // le cartelle nascoste (miniature, cestini) non contano
    const QStringList subs = QDir(dir).entryList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::NoSymLinks);
    for (const QString &s : subs)
        addTree(dir + QLatin1Char('/') + s, true, budget);
}

void Syncs::watch()
{
    const QStringList old = m_watcher.directories();
    if (!old.isEmpty())
        m_watcher.removePaths(old);
    int budget = kMaxDirs;
    for (const QVariant &v : std::as_const(m_list)) {
        const QVariantMap s = v.toMap();
        if (s.value(QStringLiteral("enabled"), true).toBool() && QFileInfo(s.value(QStringLiteral("local")).toString()).isDir())
            addTree(s.value(QStringLiteral("local")).toString(), s.value(QStringLiteral("subdirs")).toBool(), budget);
    }
}

void Syncs::onDirChanged(const QString &path)
{
    for (const QVariant &v : std::as_const(m_list)) {
        const QVariantMap s = v.toMap();
        const QString local = s.value(QStringLiteral("local")).toString();
        const QString id = s.value(kId).toString();
        if (!s.value(QStringLiteral("enabled"), true).toBool() || local.isEmpty())
            continue;
        if (path != local && !(s.value(QStringLiteral("subdirs")).toBool() && path.startsWith(local + QLatin1Char('/'))))
            continue;
        // una sottocartella nuova: si sorveglia anche lei
        if (s.value(QStringLiteral("subdirs")).toBool()) {
            int budget = kMaxDirs - int(m_watcher.directories().size());
            const QStringList subs = QDir(path).entryList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::NoSymLinks);
            for (const QString &sub : subs) {
                const QString p = path + QLatin1Char('/') + sub;
                if (!m_watcher.directories().contains(p))
                    addTree(p, true, budget);
            }
        }
        if (m_muted.contains(id))
            continue;
        QTimer *t = m_timers.value(id);
        if (!t) {
            t = new QTimer(this);
            t->setSingleShot(true);
            t->setInterval(kQuiet);
            connect(t, &QTimer::timeout, this, [this, id] {
                if (!m_muted.contains(id))
                    emit wanted(id);
            });
            m_timers.insert(id, t);
        }
        t->start();
    }
}
