#include "opener.h"

#include <QDBusConnection>
#include <QDBusError>
#include <QDBusMessage>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QLocale>
#include <QMimeDatabase>
#include <QProcess>
#include <QTextStream>
#include <QTranslator>
#include <QUrl>

#include <unistd.h>

#include "qtrust.h"

namespace {

const QString lca = QStringLiteral("/usr/bin/lca-tool");
// il testo nel visore: oltre, si avvisa che è tagliato
const qint64 maxText = 2 * 1024 * 1024;

QStringList run(const QStringList &args)
{
    QProcess p;
    p.start(lca, args);
    if (!p.waitForFinished(4000)) {
        p.kill();
        return {};
    }
    QStringList out;
    for (const QByteArray &line : p.readAllStandardOutput().split('\n'))
        if (!line.trimmed().isEmpty())
            out << QString::fromUtf8(line.trimmed());
    return out;
}

// l'icona di un'app: un percorso assoluto, oppure un nome da cercare nei temi:
// prima glacier (le icone d'acciaio di RooTundra, se c'è), poi quello di
// Sailfish alla scala più alta, poi hicolor
QString iconPath(const QString &icon)
{
    if (icon.isEmpty())
        return {};
    if (icon.startsWith(QLatin1Char('/')))
        return QFileInfo::exists(icon) ? QUrl::fromLocalFile(icon).toString() : QString();
    QStringList candidates{QStringLiteral("/usr/share/themes/glacier/meegotouch/icons/%1.png")};
    for (const char *z : {"z2.5", "z2.0", "z1.75", "z1.5", "z1.25", "z1.0"})
        candidates << QStringLiteral("/usr/share/themes/sailfish-default/silica/%1/icons/").arg(QLatin1String(z)) + QStringLiteral("%1.png");
    for (const char *size : {"172x172", "128x128", "108x108", "256x256", "86x86"})
        candidates << QStringLiteral("/usr/share/icons/hicolor/%1/apps/").arg(QLatin1String(size)) + QStringLiteral("%1.png");
    for (const QString &c : std::as_const(candidates)) {
        const QString p = c.arg(icon);
        if (QFileInfo::exists(p))
            return QUrl::fromLocalFile(p).toString();
    }
    return {};
}

// il nome tradotto delle app di Jolla: non sta in Name[it] ma nel catalogo
// X-MeeGo-Translation-Catalog, alla voce X-MeeGo-Logical-Id (come fa lipstick)
QString catalogName(const QString &catalog, const QString &logicalId)
{
    static QHash<QString, QTranslator *> loaded;
    QTranslator *&t = loaded[catalog];
    if (!t) {
        t = new QTranslator;
        const QString lang = QLocale().name();
        // gallery-it.qm, gallery_it.qm, gallery-it_IT.qm…
        bool ok = false;
        for (const QString &n : {catalog + QLatin1Char('-') + lang, catalog + QLatin1Char('_') + lang,
                                 catalog + QLatin1Char('-') + lang.left(2), catalog + QLatin1Char('_') + lang.left(2)})
            if ((ok = t->load(n, QStringLiteral("/usr/share/translations"))))
                break;
        if (!ok) {
            delete t;
            t = new QTranslator; // vuoto: non si riprova a ogni chiamata
        }
    }
    const QString tr = t->translate("", logicalId.toUtf8().constData());
    return tr.isEmpty() || tr == logicalId ? QString() : tr;
}

// le app di Sailfish girano in firejail e della home vedono solo queste
// cartelle (/etc/sailjail/permissions/UserDirs.permission), più i supporti
// rimovibili (RemovableMedia): un file altrove lo vedono «inesistente»
// (mpv sul POCO, 25/10: «Cannot open file … File o directory non esistente»)
const char *const visibleDirs[] = {"Documents", "Downloads", "Music", "Pictures", "Public", "Videos"};
const qint64 maxCopy = 200LL * 1000 * 1000;

QString bridgeDir()
{
    return QDir::homePath() + QStringLiteral("/Downloads/.rootree");
}

} // namespace

Opener::Opener(QObject *parent)
    : QObject(parent)
{
    cleanVisible();
}

void Opener::cleanVisible() const
{
    // i passaggi più vecchi di un giorno: chi li ha aperti ha finito
    QDir dir(bridgeDir());
    const QDateTime limit = QDateTime::currentDateTime().addDays(-1);
    for (const QFileInfo &fi : dir.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot))
        if (fi.lastModified() < limit)
            QDir(fi.absoluteFilePath()).removeRecursively();
}

QString Opener::visiblePath(const QString &path) const
{
    const QString abs = QFileInfo(path).absoluteFilePath();
    const QString home = QDir::homePath() + QLatin1Char('/');
    for (const char *d : visibleDirs)
        if (abs.startsWith(home + QLatin1String(d) + QLatin1Char('/')))
            return abs;
    if (abs.startsWith(QLatin1String("/run/media/")))
        return abs;
    // altrove: un collegamento fisso in ~/Downloads/.rootree/<n>/<nome> (stesso
    // file, nessuno spazio in più, le modifiche dell'app finiscono nell'originale);
    // se non si può (altro filesystem, file non nostro) una copia
    const QString dir = bridgeDir() + QLatin1Char('/') + QString::number(QDateTime::currentMSecsSinceEpoch());
    QDir().mkpath(dir);
    const QString target = dir + QLatin1Char('/') + QFileInfo(abs).fileName();
    if (::link(QFile::encodeName(abs).constData(), QFile::encodeName(target).constData()) == 0)
        return target;
    if (QFileInfo(abs).size() <= maxCopy && QFile::copy(abs, target))
        return target;
    QDir(dir).removeRecursively();
    return {};
}

QString Opener::mimeType(const QString &path) const
{
    return QMimeDatabase().mimeTypeForFile(path).name();
}

QVariantMap Opener::desktopEntry(const QString &id) const
{
    // Name[it], Name[it_IT] o Name, e Icon, dalla sezione [Desktop Entry]
    QFile f(QStringLiteral("/usr/share/applications/%1.desktop").arg(id));
    QString name, local, icon, catalog, logicalId;
    const QString lang = QLocale().name();
    const QString keyFull = QStringLiteral("Name[%1]").arg(lang);
    const QString keyShort = QStringLiteral("Name[%1]").arg(lang.left(2));
    if (f.open(QIODevice::ReadOnly)) {
        QTextStream in(&f);
        bool main = false;
        while (!in.atEnd()) {
            const QString line = in.readLine().trimmed();
            if (line.startsWith(QLatin1Char('['))) {
                main = line == QLatin1String("[Desktop Entry]");
                continue;
            }
            if (!main)
                continue;
            const int eq = line.indexOf(QLatin1Char('='));
            if (eq < 0)
                continue;
            const QString key = line.left(eq).trimmed();
            const QString value = line.mid(eq + 1).trimmed();
            if (key == QLatin1String("Name"))
                name = value;
            else if (key == keyFull || (key == keyShort && local.isEmpty()))
                local = value;
            else if (key == QLatin1String("Icon"))
                icon = value;
            else if (key == QLatin1String("X-MeeGo-Translation-Catalog"))
                catalog = value;
            else if (key == QLatin1String("X-MeeGo-Logical-Id"))
                logicalId = value;
        }
    }
    if (local.isEmpty() && !catalog.isEmpty() && !logicalId.isEmpty())
        local = catalogName(catalog, logicalId);
    return {{QStringLiteral("id"), id},
            {QStringLiteral("name"), !local.isEmpty() ? local : (!name.isEmpty() ? name : id)},
            {QStringLiteral("icon"), iconPath(icon)}};
}

QVariantList Opener::actions(const QString &path) const
{
    const QStringList ids = run({QStringLiteral("--file"), QStringLiteral("--print"), path});
    const QStringList def = run({QStringLiteral("--file"), QStringLiteral("--printdefault"), path});
    QVariantList out;
    QStringList seen;
    for (const QString &id : ids) {
        if (seen.contains(id))
            continue;
        seen << id;
        QVariantMap e = desktopEntry(id);
        e.insert(QStringLiteral("isDefault"), !def.isEmpty() && def.first() == id);
        out << e;
    }
    return out;
}

bool Opener::openDefault(const QString &path) const
{
    m_error.clear();
    if (run({QStringLiteral("--file"), QStringLiteral("--printdefault"), path}).isEmpty())
        return false;
    const QString visible = visiblePath(path);
    if (visible.isEmpty()) {
        m_error = tr("too big to open from here");
        return true; // c'è un'app, ma il file non le si può far vedere
    }
    return QProcess::startDetached(lca, {QStringLiteral("--file"), QStringLiteral("--triggerdefault"), visible});
}

void Opener::openWith(const QString &path, const QString &actionId) const
{
    m_error.clear();
    const QString visible = visiblePath(path);
    if (visible.isEmpty()) {
        m_error = tr("too big to open from here");
        return;
    }
    QProcess::startDetached(lca, {QStringLiteral("--file"), QStringLiteral("--trigger"), actionId, visible});
}

void Opener::setDefault(const QString &mime, const QString &actionId) const
{
    run({QStringLiteral("--setmimedefault"), mime, actionId});
}

bool Opener::share(const QStringList &paths) const
{
    m_error.clear();
    if (paths.isEmpty())
        return false;
    // lo stesso protocollo di ShareAction (Sailfish.Share, Qt5), ricostruito
    // come in RooTitanium: resources = i file, mimeType = il loro tipo comune
    QMimeDatabase db;
    QString mime = db.mimeTypeForFile(paths.first()).name();
    // QStringList e non QVariantList: Qt manderebbe un array di varianti (av),
    // e il dialogo di sistema accetta i file solo come array di stringhe (as):
    // con av non si apriva niente (provato a mano con gdbus, 25/10)
    QStringList resources;
    for (const QString &p : paths) {
        const QString visible = visiblePath(p);
        if (visible.isEmpty()) {
            m_error = tr("too big to open from here");
            return false;
        }
        resources << QUrl::fromLocalFile(visible).toString();
        if (db.mimeTypeForFile(p).name() != mime)
            mime = QStringLiteral("application/octet-stream");
    }
    const QVariantMap config{
        {QStringLiteral("resources"), resources},
        {QStringLiteral("mimeType"), mime},
    };
    QDBusMessage msg = QDBusMessage::createMethodCall(QStringLiteral("org.sailfishos.share"), QStringLiteral("/"),
                                                      QStringLiteral("org.sailfishos.share"), QStringLiteral("share"));
    msg.setArguments({config});
    const bool sent = QDBusConnection::sessionBus().send(msg);
    if (!sent)
        qWarning("[rootree] condivisione: messaggio D-Bus non inviato: %s",
                 qPrintable(QDBusConnection::sessionBus().lastError().message()));
    return sent;
}

QVariantMap Opener::readText(const QString &path) const
{
    try {
        const QString text = qtrust::qs(rootree::read_text(qtrust::str(path), uint64_t(maxText)));
        // un file su un server: la dimensione la sa il server, non il disco
        qint64 size = QFileInfo(path).size();
        if (path.startsWith(QStringLiteral("/@remote/"))) {
            try {
                size = qint64(rootree::stat_path(qtrust::str(path)).size);
            } catch (const rust::Error &) {
            }
        }
        return {{QStringLiteral("text"), text}, {QStringLiteral("truncated"), size > maxText}};
    } catch (const rust::Error &e) {
        const QString err = QString::fromUtf8(e.what());
        if (err == QLatin1String("binary"))
            return {{QStringLiteral("binary"), true}};
        return {{QStringLiteral("error"), err}};
    }
}
