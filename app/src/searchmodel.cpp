#include "searchmodel.h"

#include "qtrust.h"

SearchModel::SearchModel(QObject *parent)
    : QAbstractListModel(parent)
{
    m_timer.setInterval(200);
    connect(&m_timer, &QTimer::timeout, this, &SearchModel::poll);
}

SearchModel::~SearchModel()
{
    stop();
}

int SearchModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : m_items.size();
}

QVariant SearchModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_items.size())
        return {};
    const Item &it = m_items.at(index.row());
    switch (role) {
    case NameRole: return it.name;
    case PathRole: return it.path;
    case IsDirRole: return it.isDir;
    case SizeRole: return it.size;
    case MtimeRole: return it.mtime;
    case CategoryRole: return it.category;
    case SnippetRole: return it.snippet;
    case ParentRole: return it.path.left(qMax(1, it.path.lastIndexOf(QLatin1Char('/'))));
    }
    return {};
}

QHash<int, QByteArray> SearchModel::roleNames() const
{
    return {{NameRole, "name"}, {PathRole, "path"}, {IsDirRole, "isDir"}, {SizeRole, "size"}, {MtimeRole, "mtime"},
            {CategoryRole, "category"}, {SnippetRole, "snippet"}, {ParentRole, "parent"}};
}

QString SearchModel::start(const QString &root, const QString &query, int mode, bool caseSensitive, bool hidden)
{
    stop();
    clear();
    try {
        m_id = rootree::search_start(qtrust::str(root), qtrust::str(query), uint8_t(mode), caseSensitive, hidden);
    } catch (const rust::Error &e) {
        m_error = QString::fromUtf8(e.what());
        emit stateChanged();
        return m_error;
    }
    m_timer.start();
    emit stateChanged();
    return {};
}

void SearchModel::stop()
{
    if (m_id) {
        poll(); // gli ultimi arrivati
        if (m_id)
            rootree::search_stop(m_id);
        m_id = 0;
    }
    m_timer.stop();
    emit stateChanged();
}

void SearchModel::clear()
{
    beginResetModel();
    m_items.clear();
    endResetModel();
    m_found = m_scanned = 0;
    m_limited = false;
    m_current.clear();
    m_error.clear();
    emit stateChanged();
}

QVariantMap SearchModel::get(int row) const
{
    QVariantMap m;
    if (row < 0 || row >= m_items.size())
        return m;
    const QHash<int, QByteArray> roles = roleNames();
    for (auto it = roles.cbegin(); it != roles.cend(); ++it)
        m.insert(QString::fromLatin1(it.value()), data(index(row), it.key()));
    return m;
}

void SearchModel::poll()
{
    if (!m_id)
        return;
    const auto b = rootree::search_take(m_id);
    if (!b.hits.empty()) {
        const int first = m_items.size();
        beginInsertRows({}, first, first + int(b.hits.size()) - 1);
        for (const auto &h : b.hits) {
            const auto &e = h.entry;
            m_items.append({qtrust::qs(e.name), qtrust::qs(e.path), qtrust::qs(e.category), qtrust::qs(h.snippet), e.kind == 1,
                            qint64(e.size), e.mtime});
        }
        endInsertRows();
    }
    m_found = int(b.found);
    m_scanned = int(b.scanned);
    m_limited = b.limited;
    m_current = qtrust::qs(b.current);
    if (!b.error.empty())
        m_error = qtrust::qs(b.error);
    if (b.done) {
        rootree::search_stop(m_id);
        m_id = 0;
        m_timer.stop();
    }
    emit stateChanged();
}
