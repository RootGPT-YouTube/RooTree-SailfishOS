#pragma once

#include <QAbstractListModel>
#include <QFileSystemWatcher>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVector>

// Il contenuto di una cartella locale, letto dal nucleo Rust, con la
// selezione (per le operazioni su più elementi) e l'aggiornamento automatico
// quando la cartella cambia.
class FolderModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(QString path READ path WRITE setPath NOTIFY pathChanged)
    Q_PROPERTY(bool showHidden READ showHidden WRITE setShowHidden NOTIFY sortingChanged)
    Q_PROPERTY(int sortKey READ sortKey WRITE setSortKey NOTIFY sortingChanged)
    Q_PROPERTY(bool descending READ descending WRITE setDescending NOTIFY sortingChanged)
    Q_PROPERTY(bool dirsFirst READ dirsFirst WRITE setDirsFirst NOTIFY sortingChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(int dirCount READ dirCount NOTIFY countChanged)
    Q_PROPERTY(int fileCount READ fileCount NOTIFY countChanged)
    Q_PROPERTY(qint64 filesSize READ filesSize NOTIFY countChanged)
    Q_PROPERTY(int selectedCount READ selectedCount NOTIFY selectionChanged)

public:
    enum Roles {
        NameRole = Qt::UserRole + 1,
        PathRole,
        IsDirRole,
        IsLinkRole,
        LinkBrokenRole,
        SizeRole,
        MtimeRole,
        ModeRole,
        HiddenRole,
        CategoryRole,
        SelectedRole,
    };

    explicit FolderModel(QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    QString path() const { return m_path; }
    void setPath(const QString &path);
    bool showHidden() const { return m_showHidden; }
    void setShowHidden(bool v);
    int sortKey() const { return m_sortKey; }
    void setSortKey(int v);
    bool descending() const { return m_descending; }
    void setDescending(bool v);
    bool dirsFirst() const { return m_dirsFirst; }
    void setDirsFirst(bool v);
    QString error() const { return m_error; }
    int dirCount() const { return m_dirCount; }
    int fileCount() const { return m_items.size() - m_dirCount; }
    qint64 filesSize() const { return m_filesSize; }
    int selectedCount() const { return m_selected; }

    Q_INVOKABLE void refresh();
    Q_INVOKABLE QVariantMap get(int row) const;
    Q_INVOKABLE int indexOf(const QString &name) const;
    Q_INVOKABLE void setSelected(int row, bool selected);
    Q_INVOKABLE void toggleSelected(int row);
    Q_INVOKABLE void selectAll();
    Q_INVOKABLE void clearSelection();
    Q_INVOKABLE QStringList selectedPaths() const;
    Q_INVOKABLE QString coreVersion() const;

signals:
    void pathChanged();
    void sortingChanged();
    void errorChanged();
    void countChanged();
    void selectionChanged();

private:
    struct Item {
        QString name;
        QString path;
        bool isDir;
        bool isLink;
        bool linkBroken;
        qint64 size;
        qint64 mtime;
        uint mode;
        bool hidden;
        QString category;
        bool selected;
    };

    void reload(bool keepSelection);

    QString m_path;
    bool m_showHidden = false;
    int m_sortKey = 0;
    bool m_descending = false;
    bool m_dirsFirst = true;
    QString m_error;
    QVector<Item> m_items;
    int m_dirCount = 0;
    qint64 m_filesSize = 0;
    int m_selected = 0;
    QFileSystemWatcher m_watcher;
    QTimer m_changed;
};
