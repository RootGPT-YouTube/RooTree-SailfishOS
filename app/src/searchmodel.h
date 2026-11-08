#pragma once

#include <QAbstractListModel>
#include <QString>
#include <QTimer>
#include <QVector>

// I risultati di una ricerca del nucleo Rust, ritirati a intervalli mentre
// la ricerca gira (arrivano un po' alla volta, come in Dolphin).
class SearchModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(bool running READ running NOTIFY stateChanged)
    Q_PROPERTY(int found READ found NOTIFY stateChanged)
    Q_PROPERTY(int scanned READ scanned NOTIFY stateChanged)
    Q_PROPERTY(bool limited READ limited NOTIFY stateChanged)
    Q_PROPERTY(QString current READ current NOTIFY stateChanged)
    Q_PROPERTY(QString error READ error NOTIFY stateChanged)
    Q_PROPERTY(int count READ rowCount NOTIFY stateChanged)

public:
    enum Roles { NameRole = Qt::UserRole + 1, PathRole, IsDirRole, SizeRole, MtimeRole, CategoryRole, SnippetRole, ParentRole };
    // come in search.rs
    enum Mode { Name = 0, NameRegex, Content, ContentRegex };
    Q_ENUM(Mode)

    explicit SearchModel(QObject *parent = nullptr);
    ~SearchModel() override;

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    bool running() const { return m_id != 0; }
    int found() const { return m_found; }
    int scanned() const { return m_scanned; }
    bool limited() const { return m_limited; }
    QString current() const { return m_current; }
    QString error() const { return m_error; }

    // "" se partita, altrimenti l'errore (es. espressione non valida)
    Q_INVOKABLE QString start(const QString &root, const QString &query, int mode, bool caseSensitive, bool hidden);
    Q_INVOKABLE void stop();
    Q_INVOKABLE void clear();
    Q_INVOKABLE QVariantMap get(int row) const;

signals:
    void stateChanged();

private:
    void poll();

    struct Item {
        QString name, path, category, snippet;
        bool isDir;
        qint64 size, mtime;
    };
    QVector<Item> m_items;
    QTimer m_timer;
    quint64 m_id = 0;
    int m_found = 0;
    int m_scanned = 0;
    bool m_limited = false;
    QString m_current;
    QString m_error;
};
