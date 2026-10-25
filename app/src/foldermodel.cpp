#include "foldermodel.h"

#include <QDir>

// rust::Error (le funzioni che rendono Result) non è nell'estratto di cxx.h
// che il ponte generato si porta dentro: serve l'intestazione intera
#include "rust/cxx.h"
#include "rootree_core.h"

FolderModel::FolderModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int FolderModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : m_items.size();
}

QVariant FolderModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_items.size())
        return {};
    const Item &item = m_items.at(index.row());
    switch (role) {
    case NameRole: return item.name;
    case IsDirRole: return item.isDir;
    case SizeRole: return item.size;
    }
    return {};
}

QHash<int, QByteArray> FolderModel::roleNames() const
{
    return {{NameRole, "name"}, {IsDirRole, "isDir"}, {SizeRole, "size"}};
}

void FolderModel::setPath(const QString &path)
{
    const QString clean = QDir::cleanPath(path);
    if (clean == m_path)
        return;
    m_path = clean;
    emit pathChanged();
    refresh();
}

void FolderModel::setShowHidden(bool show)
{
    if (show == m_showHidden)
        return;
    m_showHidden = show;
    emit showHiddenChanged();
    refresh();
}

void FolderModel::refresh()
{
    QVector<Item> items;
    QString error;
    try {
        const QByteArray utf8 = m_path.toUtf8();
        const auto entries = rootree::list_dir(rust::Str(utf8.constData(), utf8.size()), m_showHidden);
        items.reserve(entries.size());
        for (const auto &e : entries)
            items.append({QString::fromUtf8(e.name.data(), e.name.size()), e.is_dir, e.size});
    } catch (const rust::Error &e) {
        // la cartella resta quella chiesta, vuota, con il perché
        error = QString::fromUtf8(e.what());
    }
    beginResetModel();
    m_items = items;
    endResetModel();
    emit countChanged();
    if (error != m_error) {
        m_error = error;
        emit errorChanged();
    }
}

void FolderModel::open(int row)
{
    if (row < 0 || row >= m_items.size() || !m_items.at(row).isDir)
        return;
    setPath(m_path + QLatin1Char('/') + m_items.at(row).name);
}

bool FolderModel::cdUp()
{
    if (m_path == QLatin1String("/"))
        return false;
    QDir dir(m_path);
    dir.cdUp();
    setPath(dir.absolutePath());
    return true;
}

QString FolderModel::coreVersion() const
{
    const rust::String v = rootree::version();
    return QString::fromUtf8(v.data(), v.size());
}
