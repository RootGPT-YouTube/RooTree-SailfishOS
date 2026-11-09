#pragma once

#include <QFileSystemWatcher>
#include <QHash>
#include <QSet>
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
    // la cassaforte delle password (vault.rs)
    Q_PROPERTY(bool vaultExists READ vaultExists NOTIFY vaultChanged)
    Q_PROPERTY(bool vaultUnlocked READ vaultUnlocked NOTIFY vaultChanged)

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
    // il codice a 6 cifre (Mega): vale per il prossimo collegamento, poi sparisce
    Q_INVOKABLE void setOneTimeCode(const QString &id, const QString &code) { m_oneTime.insert(id, code); }
    // in memoria per la sessione, o nella cassaforte aperta
    Q_INVOKABLE bool hasSecret(const QString &id) const;
    Q_INVOKABLE QString secret(const QString &id) const;

    // collegamento (su un thread): l'esito con connectFinished(id, code,
    // message, path). code "" = collegato; HOSTKEY_NEW, HOSTKEY_CHANGED, AUTH,
    // KEY, NET, UNSUPPORTED, ERR. message per HOSTKEY_*: «tipo:impronta»
    Q_INVOKABLE void connect(const QString &id);
    Q_INVOKABLE QString trust(const QString &id);
    Q_INVOKABLE void disconnect(const QString &id);
    // Mega: toglie la sessione ricordata; esito con megaRevoked(id, errore)
    Q_INVOKABLE void megaRevoke(const QString &id);
    // Il cestino (Nextcloud): action = list | restore | delete | empty; esito con trashDone
    Q_INVOKABLE void trash(const QString &id, const QString &action, const QString &item);
    Q_INVOKABLE bool isConnected(const QString &id) const;
    // la cartella da cui si entra: /@remote/<id> + percorso iniziale sul server
    Q_INVOKABLE QString entryPath(const QString &id) const { return m_entry.value(id); }
    // «Apri sempre da questa cartella»: il percorso sul server diventa quello
    // iniziale del profilo (e d'ingresso per la sessione)
    Q_INVOKABLE void setStartPath(const QString &id, const QString &serverPath);
    Q_INVOKABLE bool connecting(const QString &id) const { return m_connecting.contains(id); }

    // cloud: accesso dal browser (su un thread). authUrl(id, url) quando c'è
    // l'indirizzo da aprire; authFinished(id, code, message) alla fine: code ""
    // = riuscito (il token di rinnovo è il segreto della connessione, in
    // memoria; nella cassaforte con remember()), CLIENTID, CANCELLED, AUTH, NET, ERR
    Q_INVOKABLE void authorize(const QString &id);
    Q_INVOKABLE void cancelAuth(const QString &id);
    Q_INVOKABLE bool authorizing(const QString &id) const { return m_authorizing.contains(id); }
    // dimentica il segreto (accesso revocato o scaduto): si rifà il login
    Q_INVOKABLE void forgetSecret(const QString &id);

    bool vaultExists() const;
    bool vaultUnlocked() const;
    // "" se riuscita, altrimenti il perché (WRONG: password principale sbagliata)
    Q_INVOKABLE QString vaultCreate(const QString &master);
    Q_INVOKABLE QString vaultUnlock(const QString &master);
    Q_INVOKABLE void vaultLock();
    // la password di id (già in memoria) nella cassaforte aperta
    Q_INVOKABLE QString remember(const QString &id);

signals:
    void megaRevoked(const QString &id, const QString &error);
    void trashDone(const QString &id, const QString &action, const QString &error, const QString &json);
    void changed();
    void vaultChanged();
    void connectFinished(const QString &id, const QString &code, const QString &message, const QString &path);
    void authUrl(const QString &id, const QString &url);
    void authFinished(const QString &id, const QString &code, const QString &message);

private:
    void load();
    void store();
    QString file() const;

    QVariantList m_list;
    QHash<QString, QString> m_secrets;
    QHash<QString, QString> m_oneTime;
    QHash<QString, QString> m_entry;
    // il file cambiato da fuori (un'altra copia di RooTree, una modifica a
    // mano) si rilegge: mai riscriverci sopra la copia vecchia in memoria
    QFileSystemWatcher m_watcher;
    void watch();
    QSet<QString> m_connecting;
    QSet<QString> m_authorizing;
};
