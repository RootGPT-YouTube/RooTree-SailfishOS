#pragma once

#include <QFileSystemWatcher>
#include <QHash>
#include <QObject>
#include <QSet>
#include <QTimer>
#include <QVariantList>
#include <QVariantMap>

class JobManager;

// Le sincronizzazioni (0.9.5): una cartella del telefono con una cartella di
// una connessione. Si salvano in sincronizzazioni.json (configurazione
// dell'app, 600); il motore è sync.rs. Singleton «Syncs».
//
// Una sincronizzazione: {id, name, local, remote, remotePath, subdirs,
// hidden, direction (up|down|both), deletions (none|up|down|both), enabled,
// lastRun (ms), lastResult (testo breve), lastError}.
//
// Gira solo con RooTree aperta: le cartelle del telefono si sorvegliano
// (inotify via QFileSystemWatcher) e un cambio, dopo `kQuiet` ms senza altri
// cambi, chiede un giro con `wanted(id)`. Mentre gira (e un poco dopo) i
// cambi si ignorano: li fa la sincronizzazione stessa.
class Syncs : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QVariantList list READ list NOTIFY changed)

public:
    explicit Syncs(JobManager *jobs, QObject *parent = nullptr);

    QVariantList list() const { return m_list; }
    Q_INVOKABLE QVariantMap get(const QString &id) const;
    // nuova (id vuoto) o modificata; rende l'id
    Q_INVOKABLE QString save(const QVariantMap &sync);
    Q_INVOKABLE void remove(const QString &id);
    // un giro: rende l'id del lavoro (Jobs), o -1 se gira già
    Q_INVOKABLE int start(const QString &id, bool confirmed);
    Q_INVOKABLE bool running(const QString &id) const { return m_running.values().contains(id); }
    // l'esito dell'ultimo giro, da mostrare nell'elenco
    Q_INVOKABLE void setResult(const QString &id, const QString &result, bool error);
    // le sincronizzazioni di una connessione (per avvisare prima di eliminarla)
    Q_INVOKABLE QVariantList forRemote(const QString &remoteId) const;

signals:
    void changed();
    // un file è cambiato in una cartella sorvegliata
    void wanted(const QString &id);

private:
    void load();
    void store();
    QString file() const;
    QString stateDir() const;
    void watch();
    void onDirChanged(const QString &path);
    void addTree(const QString &dir, bool recursive, int &budget);

    JobManager *m_jobs;
    QVariantList m_list;
    QFileSystemWatcher m_watcher;
    // job → sincronizzazione
    QHash<quint64, QString> m_running;
    // le sincronizzazioni zitte (in corso, o appena finite)
    QSet<QString> m_muted;
    QHash<QString, QTimer *> m_timers;
};
