#pragma once

#include <QObject>
#include <QStringList>
#include <QTimer>
#include <QVariantList>
#include <QVariantMap>

// La coda delle operazioni lunghe del nucleo Rust (copia, spostamento,
// eliminazione, dimensione) e gli appunti di Copia/Taglia. Lo stato si legge
// dal nucleo a intervalli finché c'è qualcosa in corso. Singleton «Jobs».
class JobManager : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QVariantList jobs READ jobs NOTIFY jobsChanged)
    Q_PROPERTY(int activeCount READ activeCount NOTIFY jobsChanged)
    // la prima operazione che aspetta una risposta (conflitto o errore), o vuota
    Q_PROPERTY(QVariantMap question READ question NOTIFY jobsChanged)
    Q_PROPERTY(QStringList clipboard READ clipboard NOTIFY clipboardChanged)
    Q_PROPERTY(bool clipboardCut READ clipboardCut NOTIFY clipboardChanged)

public:
    // come in jobs.rs
    enum Kind { Copy = 0, Move = 1, Delete = 2, Size = 3, Extract = 4, Compress = 5, Trash = 6, Types = 7, Check = 8 };
    enum State { Queued = 0, Scanning, Running, AskConflict, AskError, Done, Failed, Cancelled };
    enum Answer { Overwrite = 0, Skip, KeepBoth, Cancel, Retry, Merge };
    Q_ENUM(Kind)
    Q_ENUM(State)
    Q_ENUM(Answer)

    explicit JobManager(QObject *parent = nullptr);

    QVariantList jobs() const { return m_jobs; }
    int activeCount() const { return m_active; }
    QVariantMap question() const;
    QStringList clipboard() const { return m_clipboard; }
    bool clipboardCut() const { return m_cut; }

    Q_INVOKABLE int copy(const QStringList &sources, const QString &dest);
    Q_INVOKABLE int move(const QStringList &sources, const QString &dest);
    // definitivo
    Q_INVOKABLE int remove(const QStringList &paths);
    // nel cestino
    Q_INVOKABLE int trash(const QStringList &paths);
    Q_INVOKABLE int size(const QStringList &paths);
    // la pagina «i»: l'occupato per tipo di file (status(id).info.byType…)
    Q_INVOKABLE int types(const QStringList &paths);
    // controlla (o ripara) la scheda montata in mount: smonta, fsck da root, rimonta
    Q_INVOKABLE int fsCheck(const QString &mount, bool repair);
    // «Estrai qui»: tutto l'archivio in dest
    Q_INVOKABLE int extract(const QString &archive, const QString &dest);
    // crea l'archivio dest (il formato dal nome) con sources
    Q_INVOKABLE int compress(const QStringList &sources, const QString &dest);
    Q_INVOKABLE QVariantMap status(int id) const;
    Q_INVOKABLE void cancel(int id);
    Q_INVOKABLE void resolve(int id, int answer, bool applyAll);
    Q_INVOKABLE void forget(int id);
    Q_INVOKABLE void forgetFinished();

    Q_INVOKABLE void setClipboard(const QStringList &paths, bool cut);
    Q_INVOKABLE void clearClipboard();
    // Incolla: copia o sposta gli appunti in dest; dopo un Taglia gli appunti si vuotano
    Q_INVOKABLE int paste(const QString &dest);

signals:
    void jobsChanged();
    void clipboardChanged();
    // un'operazione è finita (riuscita, fallita o annullata)
    void finished(int id, int kind, int state, const QString &error);

private:
    void poll();
    int started(quint64 id);

    QTimer m_timer;
    QVariantList m_jobs;
    int m_active = 0;
    QHash<quint64, int> m_lastState;
    QStringList m_clipboard;
    bool m_cut = false;
};
