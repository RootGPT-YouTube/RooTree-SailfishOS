#include "foldermodel.h"

#include <QDir>
#include <QSet>

#include "qtrust.h"

FolderModel::FolderModel(QObject *parent)
    : QAbstractListModel(parent)
{
    // una copia di 1000 file cambia la cartella 1000 volte: si rilegge una
    // volta sola, a cose ferme
    m_changed.setSingleShot(true);
    m_changed.setInterval(300);
    connect(&m_changed, &QTimer::timeout, this, [this] { reload(true); });
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, [this] { m_changed.start(); });
}

int FolderModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : m_items.size();
}

QVariant FolderModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_items.size())
        return {};
    const Item &it = m_items.at(index.row());
    switch (role) {
    case NameRole: return it.name;
    case PathRole: return it.path;
    case IsDirRole: return it.isDir;
    case IsLinkRole: return it.isLink;
    case LinkBrokenRole: return it.linkBroken;
    case SizeRole: return it.size;
    case MtimeRole: return it.mtime;
    case ModeRole: return it.mode;
    case HiddenRole: return it.hidden;
    case CategoryRole: return it.category;
    case SelectedRole: return it.selected;
    }
    return {};
}

QHash<int, QByteArray> FolderModel::roleNames() const
{
    return {
        {NameRole, "name"}, {PathRole, "path"}, {IsDirRole, "isDir"}, {IsLinkRole, "isLink"},
        {LinkBrokenRole, "linkBroken"}, {SizeRole, "size"}, {MtimeRole, "mtime"}, {ModeRole, "mode"},
        {HiddenRole, "hidden"}, {CategoryRole, "category"}, {SelectedRole, "selected"},
    };
}

void FolderModel::setPath(const QString &path)
{
    const QString clean = QDir::cleanPath(path);
    if (clean == m_path)
        return;
    if (!m_path.isEmpty())
        m_watcher.removePath(m_path);
    m_path = clean;
    emit pathChanged();
    reload(false);
    m_watcher.addPath(m_path);
}

#define SORT_SETTER(name, member, type)        \
    void FolderModel::name(type v)             \
    {                                          \
        if (v == member)                       \
            return;                            \
        member = v;                            \
        emit sortingChanged();                 \
        reload(true);                          \
    }
SORT_SETTER(setShowHidden, m_showHidden, bool)
SORT_SETTER(setSortKey, m_sortKey, int)
SORT_SETTER(setDescending, m_descending, bool)
SORT_SETTER(setDirsFirst, m_dirsFirst, bool)
#undef SORT_SETTER

void FolderModel::refresh()
{
    reload(true);
}

void FolderModel::reload(bool keepSelection)
{
    QSet<QString> wasSelected;
    if (keepSelection) {
        for (const Item &it : std::as_const(m_items))
            if (it.selected)
                wasSelected.insert(it.name);
    }
    QVector<Item> items;
    QString error;
    int dirs = 0;
    qint64 bytes = 0;
    try {
        const auto entries = rootree::list_dir(qtrust::str(m_path), m_showHidden, uint8_t(m_sortKey),
                                               m_descending, m_dirsFirst);
        items.reserve(entries.size());
        for (const auto &e : entries) {
            const bool isDir = e.kind == 1;
            const QString name = qtrust::qs(e.name);
            items.append({name, qtrust::qs(e.path), isDir, e.is_link, e.link_broken, qint64(e.size),
                          e.mtime, e.mode, e.hidden, qtrust::qs(e.category), wasSelected.contains(name)});
            if (isDir)
                ++dirs;
            else
                bytes += qint64(e.size);
        }
    } catch (const rust::Error &e) {
        // la cartella resta quella chiesta, vuota, con il perché
        error = QString::fromUtf8(e.what());
    }
    beginResetModel();
    m_items = items;
    m_dirCount = dirs;
    m_filesSize = bytes;
    m_selected = 0;
    for (const Item &it : std::as_const(m_items))
        m_selected += it.selected;
    endResetModel();
    emit countChanged();
    emit selectionChanged();
    if (error != m_error) {
        m_error = error;
        emit errorChanged();
    }
}

QVariantMap FolderModel::get(int row) const
{
    QVariantMap m;
    if (row < 0 || row >= m_items.size())
        return m;
    const QHash<int, QByteArray> roles = roleNames();
    for (auto it = roles.cbegin(); it != roles.cend(); ++it)
        m.insert(QString::fromLatin1(it.value()), data(index(row), it.key()));
    return m;
}

int FolderModel::indexOf(const QString &name) const
{
    for (int i = 0; i < m_items.size(); ++i)
        if (m_items.at(i).name == name)
            return i;
    return -1;
}

void FolderModel::setSelected(int row, bool selected)
{
    if (row < 0 || row >= m_items.size() || m_items.at(row).selected == selected)
        return;
    m_items[row].selected = selected;
    m_selected += selected ? 1 : -1;
    emit dataChanged(index(row), index(row), {SelectedRole});
    emit selectionChanged();
}

void FolderModel::toggleSelected(int row)
{
    if (row >= 0 && row < m_items.size())
        setSelected(row, !m_items.at(row).selected);
}

void FolderModel::selectAll()
{
    for (Item &it : m_items)
        it.selected = true;
    m_selected = m_items.size();
    if (!m_items.isEmpty())
        emit dataChanged(index(0), index(m_items.size() - 1), {SelectedRole});
    emit selectionChanged();
}

void FolderModel::clearSelection()
{
    if (m_selected == 0)
        return;
    for (Item &it : m_items)
        it.selected = false;
    m_selected = 0;
    emit dataChanged(index(0), index(m_items.size() - 1), {SelectedRole});
    emit selectionChanged();
}

QStringList FolderModel::selectedPaths() const
{
    QStringList out;
    for (const Item &it : m_items)
        if (it.selected)
            out << it.path;
    return out;
}

QString FolderModel::coreVersion() const
{
    return qtrust::qs(rootree::version());
}
