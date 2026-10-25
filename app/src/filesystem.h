#pragma once

#include <QObject>
#include <QSettings>
#include <QStringList>
#include <QVariantList>
#include <QVariantMap>

// Le operazioni immediate per il QML (memorie, informazioni, nuova cartella,
// rinomina, permessi) e i segnalibri. Esposto come singleton «Fs».
class FileSystem : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QString home READ home CONSTANT)
    Q_PROPERTY(QVariantList bookmarks READ bookmarks NOTIFY bookmarksChanged)

public:
    explicit FileSystem(QObject *parent = nullptr);

    QString home() const;
    QVariantList bookmarks() const;

    // [{kind, label, path, total, free}], kind come Place in lib.rs
    Q_INVOKABLE QVariantList places() const;
    Q_INVOKABLE QVariantMap info(const QString &path) const;
    // "" se riuscito, altrimenti il messaggio d'errore
    Q_INVOKABLE QString makeDir(const QString &parent, const QString &name) const;
    Q_INVOKABLE QString rename(const QString &path, const QString &newName) const;
    Q_INVOKABLE QString setMode(const QString &path, int mode) const;
    Q_INVOKABLE bool exists(const QString &path) const;
    Q_INVOKABLE bool isWritable(const QString &path) const;
    Q_INVOKABLE QString parentOf(const QString &path) const;
    Q_INVOKABLE QString join(const QString &dir, const QString &name) const;

    Q_INVOKABLE bool isBookmarked(const QString &path) const;
    Q_INVOKABLE void addBookmark(const QString &path);
    Q_INVOKABLE void removeBookmark(const QString &path);

signals:
    void bookmarksChanged();

private:
    QStringList m_bookmarks;
    QSettings m_settings;
};
