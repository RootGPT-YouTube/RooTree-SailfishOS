"""Banco di prova delle sincronizzazioni e del menù a tendine (0.9.5), sul PC,
con tocchi veri: carica il Main.qml vero con i singleton C++ finti (stub/) e lo
guida con QTest.touchEvent. Vedi LEGGIMI.md."""
import os, sys, tempfile

B = os.path.dirname(os.path.abspath(__file__))
QML = os.path.normpath(os.path.join(B, "..", "..", "app", "qml"))
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
# le impostazioni (Settings di QtCore) in una cartella usa e getta, mai quelle vere
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="rootree-banco-")

from PySide6.QtCore import QUrl, QPoint, QPointF, qInstallMessageHandler
from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine, QJSValue
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest

LOG = []
def _h(mode, ctx, msg):
    LOG.append(msg)
    if os.environ.get("BANCO_LOG"): print("QML", msg, flush=True)
    if "rror" in msg or "is not a function" in msg or "undefined" in msg and "TypeError" in msg:
        print("LOG", msg, flush=True)
qInstallMessageHandler(_h)

app = QGuiApplication(sys.argv)
app.setOrganizationName("rootree-banco")
app.setApplicationName("banco")
eng = QQmlApplicationEngine()
eng.addImportPath(os.path.join(B, "stub"))
FOTO = "/home/defaultuser/Pictures/Foto"
eng.rootContext().setContextProperty("startPath", FOTO)
eng.load(QUrl.fromLocalFile(os.path.join(QML, "Main.qml")))
if not eng.rootObjects():
    sys.exit("QML non caricato")
w = eng.rootObjects()[0]
w.setWidth(450)
w.setHeight(900)
dev = QTest.createTouchDevice()

def single(name):
    return eng.singletonInstance("RooTree", name)
Syncs, Remotes, Jobs = single("Syncs"), single("Remotes"), single("Jobs")

def val(v):
    if isinstance(v, QJSValue):
        return v.toVariant()
    return v

def wait(ms=60):
    QTest.qWait(ms)

def items(root=None):
    root = root or w.contentItem()
    out = [root]
    for c in root.childItems():
        out += items(c)
    return out

def shown(it):
    """visibile davvero: lui e tutti i padri visibili e non trasparenti"""
    p = it
    while p is not None:
        if not p.isVisible() or p.opacity() <= 0.01:
            return False
        p = p.parentItem()
    return it.width() > 0 and it.height() > 0

def find(text, cls=None, exact=True):
    hits = []
    for it in items():
        t = it.property("text")
        if not isinstance(t, str):
            continue
        if (t == text if exact else text in t) and shown(it):
            if cls and cls not in it.metaObject().className():
                continue
            hits.append(it)
    # la riga intera (MenuRow, pulsante) è il più largo fra quelli col testo
    hits.sort(key=lambda i: -i.width() * i.height())
    return hits[0] if hits else None

def fields():
    """le caselle di testo visibili (TextField e i suoi derivati)"""
    out = []
    for it in items():
        cn = it.metaObject().className()
        if (it.property("placeholderText") is not None and it.property("selectByMouse") is not None
                and isinstance(it.property("text"), str) and shown(it)):
            out.append(it)
    return out

def find_prop(name, value):
    for it in items():
        if it.property(name) == value and shown(it):
            return it
    return None

def center(it):
    p = it.mapToScene(QPointF(it.width() / 2, it.height() / 2))
    return int(p.x()), int(p.y())

def tap_at(x, y):
    QTest.touchEvent(w, dev).press(0, QPoint(x, y), w).commit()
    wait(40)
    QTest.touchEvent(w, dev).release(0, QPoint(x, y), w).commit()
    wait(250)

def flick_of(it):
    p = it.parentItem()
    while p is not None:
        if "Flickable" in p.metaObject().className() or "ListView" in p.metaObject().className():
            return p
        p = p.parentItem()
    return None

def drag(x, y, dy):
    QTest.touchEvent(w, dev).press(0, QPoint(x, y), w).commit(); wait(16)
    for k in range(1, 16):
        QTest.touchEvent(w, dev).move(0, QPoint(x, int(y + dy * k / 15)), w).commit(); wait(16)
    # il dito si ferma prima di staccarsi: niente lancio, il pannello resta dov'è
    for _ in range(4):
        QTest.touchEvent(w, dev).move(0, QPoint(x, int(y + dy)), w).commit(); wait(60)
    QTest.touchEvent(w, dev).release(0, QPoint(x, int(y + dy)), w).commit(); wait(500)

def into_view(it):
    """come il dito: si scorre il pannello finché la riga non è dentro"""
    for _ in range(8):
        f = flick_of(it)
        if f is None:
            return
        top = f.mapToScene(QPointF(0, 0)).y()
        bottom = top + f.height()
        y = it.mapToScene(QPointF(0, it.height() / 2)).y()
        if top + 10 < y < bottom - 10:
            return
        fx = int(f.mapToScene(QPointF(f.width() / 2, 0)).x())
        mid = int(top + f.height() / 2)
        step = max(-f.height() * 0.6, min(f.height() * 0.6, (top + bottom) / 2 - y))
        drag(fx, mid, int(step))

def tap(text, exact=True):
    it = find(text, exact=exact)
    if it is None:
        fail(f"non trovo «{text}» sullo schermo")
        return False
    into_view(it)
    tap_at(*center(it))
    return True

SHOTS = os.environ.get("BANCO_SHOTS")
def shot(name):
    """con BANCO_SHOTS=<cartella>: uno scatto della finestra, da guardare"""
    if SHOTS:
        wait(300)
        w.grabWindow().save(os.path.join(SHOTS, name + ".png"))

ok = True
def fail(msg):
    global ok
    ok = False
    print("✗", msg, flush=True)

def check(cond, msg):
    if cond:
        print("✓", msg, flush=True)
    else:
        fail(msg)

def visible(text, exact=True):
    return find(text, exact=exact) is not None

def menu():
    b = find_prop("icon", "menu")
    tap_at(*center(b))
    wait(300)

def close_popups():
    # un tocco in alto fuori dai pannelli
    tap_at(225, 20)
    wait(300)

wait(3000)  # il giro d'apertura (nessuna sincronizzazione: niente)

# ── 1 · menù ⋮ a tendine ─────────────────────────────────────────────────
menu()
shot("0-menu")
check(visible("Visualizzazione") and visible("Opzioni cartella"), "menù: ci sono «Visualizzazione» e «Opzioni cartella»")
check(not visible("Dettagli") and not visible("Nuova cartella") and not visible("Prima i più vecchi"), "menù: le tendine si aprono chiuse")
tap("Visualizzazione")
check(visible("Icone") and visible("Dettagli") and visible("Ordina per".upper()) is not None, "Visualizzazione aperta: Icone, Dettagli…")
check(visible("Nome") and visible("Cartelle prima") and visible("Solo per questa cartella"), "Visualizzazione aperta: anche l'ordinamento")
tap("Dettagli")
menu()
row = find("Visualizzazione")
check(row is not None and row.property("detail") == "Dettagli · Data", "Visualizzazione chiusa mostra «Dettagli · Data» (%s)" % (row.property("detail") if row else None))
tap("Opzioni cartella")
check(visible("Nuova cartella") and visible("Nuovo file") and visible("Cestino") and visible("Seleziona tutto"), "Opzioni cartella: le azioni della cartella")
check(visible("Sincronizza questa cartella…"), "Opzioni cartella: «Sincronizza questa cartella…»")
shot("1-menu-opzioni-cartella")

# ── 2 · procedura guidata ──────────────────────────────────────────────────
tap("Sincronizza questa cartella…")
wait(300)
check(visible("Nuova sincronizzazione"), "la procedura si apre")
shot("2-passo1")
check(visible("Foto") and visible("Memoria interna/Pictures/Foto"), "passo 1: la cartella del telefono è quella aperta")
tap("Avanti")
check(visible("Con quale memoria di rete o cloud?"), "passo 2: la scelta della connessione")
avanti = find("Avanti")
check(avanti is not None and not avanti.isEnabled(), "passo 2: Avanti spento finché non si sceglie")
tap("NAS Remoto")
shot("3-passo2")
tap("Avanti")
shot("4-passo3")
field = fields()
check(field and field[0].property("text") == "/Foto", "passo 3: cartella remota proposta «/Foto» (%s)" % (field[0].property("text") if field else None))
tap("Sfoglia…")
wait(300)
check(visible("Apri la cassaforte"), "Sfoglia: la connessione chiede la cassaforte")
pw = [i for i in fields() if i.hasActiveFocus()]
check(len(pw) >= 1, "c'è il campo della password principale")
if pw:
    pw[0].setProperty("text", "giusta")
tap("Apri")
wait(500)
check(val(Remotes.property("connectCalls")) == ["nas"], "collegamento chiesto a «nas» (%s)" % val(Remotes.property("connectCalls")))
check(visible("Usa questa cartella"), "si apre il selettore della cartella remota")
shot("5-selettore")
check(find("NAS Remoto") is not None, "il selettore porta il nome della connessione")
tap("Album")
tap("Usa questa cartella")
field = fields()
check(field and field[0].property("text") == "/Foto/Album", "la cartella scelta torna nel campo (%s)" % (field[0].property("text") if field else None))
check(visible("Nuova sincronizzazione"), "dopo il collegamento si resta nella procedura (non si entra nel NAS)")
tap("Avanti")
check(visible("DIREZIONE"), "passo 4: le regole")
check(visible("Bivalente") and visible("Da Locale") and visible("Da Remoto"), "passo 4: le tre direzioni coi loro nomi")
check(visible("Telefono → NAS Remoto") and visible("NAS Remoto → Telefono") and visible("Telefono ⇄ NAS Remoto"), "passo 4: il verso disegnato per ognuna")
tap("Bivalente")
tap("Eliminati sul remoto ⇒ anche sul telefono")
check(visible("al giro dopo torna</b> sul telefono", exact=False), "Cosa succederà (Bivalente): eliminato dal telefono ⇒ torna dal NAS")
check(visible("Elimini un file da «NAS Remoto»</b> → viene eliminato anche dal telefono", exact=False), "Cosa succederà: eliminato dal NAS ⇒ via anche dal telefono")
# il caso dell'utente: Da Locale + eliminazioni dal remoto
tap("Da Locale")
check(visible("Aggiungi un file su «NAS Remoto»</b> → resta solo su «NAS Remoto»", exact=False), "Cosa succederà (Da Locale): un file nuovo sul NAS resta lì")
check(visible("Modifichi un file sul telefono</b> → la nuova versione va su «NAS Remoto»", exact=False), "Cosa succederà (Da Locale): la modifica sul telefono va sul NAS")
check(visible("Elimini un file da «NAS Remoto»</b> → viene eliminato anche dal telefono", exact=False), "Cosa succederà (Da Locale): archiviato sul NAS ⇒ sparisce dal telefono")
shot("6b-da-locale")
cs = find("Cosa succederà")
if cs:
    into_view(cs)
    drag(225, 600, -250)
shot("6c-cosa-succedera")
tap("Da Remoto")
check(visible("Aggiungi un file sul telefono</b> → resta solo sul telefono", exact=False), "Cosa succederà (Da Remoto): un file nuovo sul telefono resta lì")
tap("Bivalente")
shot("6-regole")
check(visible("Salvaguardie.", exact=False), "passo 4: spiegate le salvaguardie")
check(visible("solo con RooTree aperta", exact=False), "passo 4: spiegato che gira solo con RooTree aperta")
tap("Avanti")
name = fields()
check(name and name[0].property("text") == "Foto ⇄ NAS Remoto", "passo 5: nome proposto «Foto ⇄ NAS Remoto» (%s)" % (name[0].property("text") if name else None))
check(visible("Telefono: Memoria interna/Pictures/Foto (con le sottocartelle)"), "riepilogo: telefono con le sottocartelle")
check(visible("Eliminazioni: dal remoto al telefono"), "riepilogo: eliminazioni dal remoto al telefono")
shot("7-riepilogo")
tap("Salva")
wait(400)
lst = val(Syncs.property("list"))
s = lst[0] if lst else {}
check(len(lst) == 1 and s.get("local") == FOTO and s.get("remote") == "nas" and s.get("remotePath") == "/Foto/Album"
      and s.get("direction") == "both" and s.get("deletions") == "down" and s.get("subdirs") is True and s.get("enabled") is True,
      "salvata giusta: %s" % s)
st = val(Syncs.property("started"))
check(st == [{"id": "s1", "confirmed": False}], "primo giro partito subito (%s)" % st)

# ── 3 · salvaguardia: cartella remota vuota tutta insieme ──────────────────
Jobs.fakeFinish(100, 5, {"syncId": "s1", "confirm": "empty-remote", "pending": 2}, "")
wait(400)
check(visible("adesso è vuota", exact=False), "la conferma spiega la cartella remota vuota")
shot("8-conferma")
tap("Conferma")
wait(300)
st = val(Syncs.property("started"))
check(st and st[-1] == {"id": "s1", "confirmed": True}, "confermato: giro con le eliminazioni (%s)" % st[-1:])

# ── 4 · esito ──────────────────────────────────────────────────────────────
Jobs.fakeFinish(101, 5, {"syncId": "s1", "uploaded": 3, "downloaded": 1, "deletedLocal": 2, "skipped": []}, "")
wait(300)
res = val(Syncs.property("results")).get("s1", {})
check(res.get("text") == "3 caricati, 1 scaricato, 2 tolti dal telefono" and not res.get("error"), "esito annotato: %s" % res)
check(visible("«Foto ⇄ NAS Remoto»: 3 caricati, 1 scaricato, 2 tolti dal telefono"), "avviso con l'esito")

# ── 5 · destinazione irraggiungibile: avviso volatile ───────────────────────
wait(3000)
Remotes.setProperty("connectedIds", [])
Remotes.setProperty("nextCode", "NET")
menu()
tap("Opzioni per Remoto")
tap("Sincronizza ora")
wait(600)
t = find("Impossibile sincronizzare «Foto ⇄ NAS Remoto»: non raggiungibile")
check(t is not None, "irraggiungibile: avviso «Impossibile sincronizzare»")
wait(5600)
check(find("Impossibile sincronizzare «Foto ⇄ NAS Remoto»: non raggiungibile") is None, "l'avviso sparisce da solo dopo 5 s")
res = val(Syncs.property("results")).get("s1", {})
check(res.get("error") is True and "non raggiungibile" in res.get("text", ""), "fallimento annotato nell'elenco: %s" % res)

# ── 6 · elenco delle sincronizzazioni ──────────────────────────────────────
Remotes.setProperty("nextCode", "")
menu()
tap("Opzioni per Remoto")
tap("Sincronizzazioni…")
wait(300)
check(visible("Foto ⇄ NAS Remoto"), "elenco: c'è la sincronizzazione")
shot("9-elenco")
check(visible("non riuscita: non raggiungibile", exact=False), "elenco: mostra l'ultimo esito")
tap("Foto ⇄ NAS Remoto")
wait(300)
check(visible("Sincronizza ora") and visible("Modifica") and visible("Elimina"), "le azioni della sincronizzazione")
n = len(val(Syncs.property("started")))
tap("Sincronizza ora")
wait(600)
check(len(val(Syncs.property("started"))) == n + 1, "«Sincronizza ora» dall'elenco fa un giro")
menu()
tap("Opzioni per Remoto")
tap("Sincronizzazioni…")
tap("Foto ⇄ NAS Remoto")
tap("Modifica")
wait(300)
check(visible("Modifica sincronizzazione"), "Modifica riapre la procedura")
tap("Avanti"); tap("Avanti"); tap("Avanti"); tap("Avanti")
name = fields()
check(name and name[0].property("text") == "Foto ⇄ NAS Remoto", "in modifica il nome resta")
tap("Salva")
wait(300)
check(len(val(Syncs.property("list"))) == 1, "modificata, non duplicata")
menu()
tap("Opzioni per Remoto")
tap("Sincronizzazioni…")
tap("Foto ⇄ NAS Remoto")
tap("Elimina")
wait(300)
check(visible("Eliminare la sincronizzazione?"), "eliminare chiede conferma")
tap("Elimina")
wait(300)
check(len(val(Syncs.property("list"))) == 0, "eliminata")

bad = [m for m in LOG if "TypeError" in m or "ReferenceError" in m or "is not a function" in m]
check(not bad, "nessun errore JavaScript nel QML (%d)" % len(bad))
for m in bad[:10]:
    print("   ", m)
print("TUTTO OK" if ok else "CI SONO ERRORI")
sys.exit(0 if ok else 1)
