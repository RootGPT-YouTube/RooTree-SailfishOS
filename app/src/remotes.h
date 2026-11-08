#pragma once

#include <QHash>
#include <QObject>
#include <QVariantList>
#include <QVariantMap>

// Le connessioni remote salvate (Fase 5): profili di server e cloud, in un
// file JSON nella configurazione dell'app (permessi 600). Singleton «Remotes».
//
// Un profilo: {id, type, name, host, port, path, user, auth, keyPath,
// secure, share, domain, bucket, region, endpoint, clientId}. Le password NON
// stanno nel file: per ora restano in memoria per la sessione (setSecret);
// la cassaforte cifrata arriva con la connessione vera.
class Remotes : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QVariantList list READ list NOTIFY changed)
    Q_PROPERTY(QVariantList types READ types CONSTANT)

public:
    explicit Remotes(QObject *parent = nullptr);

    QVariantList list() const { return m_list; }
    // i tipi che la procedura offre: {type, label, group, icon, port, auths, needsServer, oauth, note}
    QVariantList types() const;

    Q_INVOKABLE QVariantMap typeInfo(const QString &type) const;
    Q_INVOKABLE QVariantMap get(const QString &id) const;
    // nuovo (id vuoto) o modificato; rende l'id
    Q_INVOKABLE QString save(const QVariantMap &profile);
    Q_INVOKABLE void remove(const QString &id);
    Q_INVOKABLE QString duplicate(const QString &id);
    // «utente@host:porta/percorso», o il nome del servizio per i cloud
    Q_INVOKABLE QString describe(const QVariantMap &profile) const;

    Q_INVOKABLE void setSecret(const QString &id, const QString &secret) { m_secrets.insert(id, secret); }
    Q_INVOKABLE bool hasSecret(const QString &id) const { return m_secrets.contains(id); }

signals:
    void changed();

private:
    void load();
    void store();
    QString file() const;

    QVariantList m_list;
    QHash<QString, QString> m_secrets;
};
