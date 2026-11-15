#include "jobmanager.h"

#include "qtrust.h"

#include <QJsonDocument>

JobManager::JobManager(QObject *parent)
    : QObject(parent)
{
    // abbastanza spesso per una barra che scorre liscia, abbastanza di rado
    // da non pesare: il timer gira solo mentre c'è un'operazione aperta
    m_timer.setInterval(150);
    connect(&m_timer, &QTimer::timeout, this, &JobManager::poll);
}

int JobManager::started(quint64 id)
{
    m_timer.start();
    poll();
    return int(id);
}

int JobManager::copy(const QStringList &sources, const QString &dest)
{
    return started(rootree::job_copy(qtrust::vec(sources), qtrust::str(dest)));
}

int JobManager::move(const QStringList &sources, const QString &dest)
{
    return started(rootree::job_move(qtrust::vec(sources), qtrust::str(dest)));
}

int JobManager::trash(const QStringList &paths)
{
    return started(rootree::job_trash(qtrust::vec(paths)));
}

int JobManager::remove(const QStringList &paths)
{
    return started(rootree::job_delete(qtrust::vec(paths)));
}

int JobManager::size(const QStringList &paths)
{
    return started(rootree::job_size(qtrust::vec(paths)));
}

int JobManager::types(const QStringList &paths)
{
    return started(rootree::job_types(qtrust::vec(paths)));
}

int JobManager::fsCheck(const QString &mount, bool repair)
{
    return started(rootree::job_fs_check(qtrust::str(mount), repair));
}

int JobManager::extract(const QString &archive, const QString &dest)
{
    return started(rootree::job_extract(qtrust::str(archive), qtrust::str(dest)));
}

int JobManager::compress(const QStringList &sources, const QString &dest)
{
    return started(rootree::job_compress(qtrust::vec(sources), qtrust::str(dest)));
}

void JobManager::cancel(int id)
{
    rootree::job_cancel(quint64(id));
    poll();
}

void JobManager::resolve(int id, int answer, bool applyAll)
{
    rootree::job_resolve(quint64(id), uint8_t(answer), applyAll);
    poll();
}

void JobManager::forget(int id)
{
    rootree::job_forget(quint64(id));
    m_lastState.remove(quint64(id));
    poll();
}

void JobManager::forgetFinished()
{
    for (const QVariant &v : std::as_const(m_jobs)) {
        const QVariantMap j = v.toMap();
        if (j.value(QStringLiteral("state")).toInt() >= Done)
            rootree::job_forget(j.value(QStringLiteral("id")).toULongLong());
    }
    poll();
}

void JobManager::poll()
{
    QVariantList list;
    int active = 0;
    for (const auto &s : rootree::job_statuses()) {
        const int state = int(s.state);
        QVariantMap job{
            {QStringLiteral("id"), int(s.id)},
            {QStringLiteral("kind"), int(s.kind)},
            {QStringLiteral("state"), state},
            {QStringLiteral("totalBytes"), qint64(s.total_bytes)},
            {QStringLiteral("doneBytes"), qint64(s.done_bytes)},
            {QStringLiteral("totalItems"), qint64(s.total_items)},
            {QStringLiteral("doneItems"), qint64(s.done_items)},
            {QStringLiteral("dirs"), qint64(s.dirs)},
            {QStringLiteral("current"), qtrust::qs(s.current)},
            {QStringLiteral("error"), qtrust::qs(s.error)},
            {QStringLiteral("askSrc"), qtrust::qs(s.ask_src)},
            {QStringLiteral("askDst"), qtrust::qs(s.ask_dst)},
            {QStringLiteral("askDstIsDir"), s.ask_dst_is_dir},
            {QStringLiteral("askSrcIsDir"), s.ask_src_is_dir},
            {QStringLiteral("dest"), qtrust::qs(s.dest)},
            {QStringLiteral("firstSource"), qtrust::qs(s.first_source)},
            {QStringLiteral("sources"), qint64(s.sources)},
        };
        // i dati in più (per tipo di file, esito del controllo), già decodificati
        if (!s.detail.empty())
            job.insert(QStringLiteral("info"), QJsonDocument::fromJson(QByteArray(s.detail.data(), qsizetype(s.detail.size()))).toVariant());
        list << job;
        if (state < Done) {
            // quelli della pagina «i» non sono operazioni dell'utente sui file
            if (s.kind != Size && s.kind != Types && s.kind != Check)
                ++active;
        } else if (m_lastState.value(s.id, -1) != state) {
            emit finished(int(s.id), int(s.kind), state, qtrust::qs(s.error));
        }
        m_lastState.insert(s.id, state);
    }
    m_jobs = list;
    m_active = active;
    bool open = false;
    for (const QVariant &v : std::as_const(m_jobs))
        open |= v.toMap().value(QStringLiteral("state")).toInt() < Done;
    if (!open)
        m_timer.stop();
    emit jobsChanged();
}

QVariantMap JobManager::status(int id) const
{
    for (const QVariant &v : m_jobs)
        if (v.toMap().value(QStringLiteral("id")).toInt() == id)
            return v.toMap();
    return {};
}

QVariantMap JobManager::question() const
{
    for (const QVariant &v : m_jobs) {
        const int state = v.toMap().value(QStringLiteral("state")).toInt();
        if (state == AskConflict || state == AskError)
            return v.toMap();
    }
    return {};
}

void JobManager::setClipboard(const QStringList &paths, bool cut)
{
    m_clipboard = paths;
    m_cut = cut;
    emit clipboardChanged();
}

void JobManager::clearClipboard()
{
    setClipboard({}, false);
}

int JobManager::paste(const QString &dest)
{
    if (m_clipboard.isEmpty())
        return 0;
    const int id = m_cut ? move(m_clipboard, dest) : copy(m_clipboard, dest);
    // dopo Taglia i file non sono più dove dicono gli appunti
    if (m_cut)
        clearClipboard();
    return id;
}
