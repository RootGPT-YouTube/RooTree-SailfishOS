#pragma once

#include <QObject>
#include <QStringList>
#include <QVariantList>
#include <QVariantMap>

// Aprire i file con le app di Sailfish e condividerli. «Apri con» passa da
// libcontentaction (lo strumento lca-tool), lo stesso meccanismo delle app di
// sistema; la condivisione dal dialogo di sistema (org.sailfishos.share).
// Singleton «Opener».
class Opener : public QObject
{
    Q_OBJECT

public:
    explicit Opener(QObject *parent = nullptr);

    Q_INVOKABLE QString mimeType(const QString &path) const;
    // [{id, name, icon, isDefault}]: le app che aprono questo file
    Q_INVOKABLE QVariantList actions(const QString &path) const;
    // false se non c'è un'app predefinita per questo tipo
    Q_INVOKABLE bool openDefault(const QString &path) const;
    Q_INVOKABLE void openWith(const QString &path, const QString &actionId) const;
    Q_INVOKABLE void setDefault(const QString &mime, const QString &actionId) const;
    Q_INVOKABLE bool share(const QStringList &paths) const;
    // il testo per il visore: {text, truncated} oppure {binary: true} / {error}
    Q_INVOKABLE QVariantMap readText(const QString &path) const;

    // l'ultimo problema nell'aprire o condividere (per il QML), o ""
    Q_INVOKABLE QString lastError() const { return m_error; }

private:
    QVariantMap desktopEntry(const QString &id) const;
    // un percorso che le app nella sandbox di Sailfish possono leggere
    QString visiblePath(const QString &path) const;
    void cleanVisible() const;

    mutable QString m_error;
};
