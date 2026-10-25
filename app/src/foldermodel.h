#pragma once

#include <QAbstractListModel>
#include <QString>
#include <QVector>

// Il contenuto di una cartella locale, letto dal nucleo Rust.
class FolderModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(QString path READ path WRITE setPath NOTIFY pathChanged)
    Q_PROPERTY(bool showHidden READ showHidden WRITE setShowHidden NOTIFY showHiddenChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)

public:
    enum Roles { NameRole = Qt::UserRole + 1, IsDirRole, SizeRole };

    explicit FolderModel(QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    QString path() const { return m_path; }
    void setPath(const QString &path);
    bool showHidden() const { return m_showHidden; }
    void setShowHidden(bool show);
    QString error() const { return m_error; }

    Q_INVOKABLE void open(int row);
    Q_INVOKABLE bool cdUp();
    Q_INVOKABLE void refresh();
    Q_INVOKABLE QString coreVersion() const;

signals:
    void pathChanged();
    void showHiddenChanged();
    void errorChanged();
    void countChanged();

private:
    struct Item {
        QString name;
        bool isDir;
        quint64 size;
    };

    QString m_path;
    bool m_showHidden = false;
    QString m_error;
    QVector<Item> m_items;
};
