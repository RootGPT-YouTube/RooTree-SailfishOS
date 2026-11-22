"""Banco di prova delle due schede A/B (0.9.8), sul PC, con tocchi veri: carica
il Main.qml vero con i singleton finti di collaudo-sync/stub e lo guida con
QTest.touchEvent (tocchi sulle linguette, swipe sulla cartella). Vedi LEGGIMI.md."""
import os, sys, tempfile

B = os.path.dirname(os.path.abspath(__file__))
QML = os.path.normpath(os.path.join(B, "..", "..", "app", "qml"))
STUB = os.path.normpath(os.path.join(B, "..", "collaudo-sync", "stub"))
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
# le impostazioni (Settings di QtCore) in una cartella usa e getta, mai quelle vere
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp(prefix="rootree-banco-")

from PySide6.QtCore import QUrl, QPoint, QPointF, qInstallMessageHandler
from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtTest import QTest
from PySide6.QtQuick import QQuickWindow
import shiboken6

LOG = []
def _h(mode, ctx, msg):
    LOG.append(msg)
    if os.environ.get("BANCO_LOG"): print("QML", msg, flush=True)
qInstallMessageHandler(_h)

app = QGuiApplication(sys.argv)
app.setOrganizationName("rootree-banco")
app.setApplicationName("banco")
eng = QQmlApplicationEngine()
eng.addImportPath(STUB)
HOME = "/home/defaultuser"
FOTO = HOME + "/Pictures/Foto"
eng.rootContext().setContextProperty("startPath", FOTO)
eng.load(QUrl.fromLocalFile(os.path.join(QML, "Main.qml")))
if not eng.rootObjects():
    sys.exit("QML non caricato")
w = eng.rootObjects()[0]
w.setWidth(450)
w.setHeight(900)
dev = QTest.createTouchDevice()
SHOTS = os.environ.get("BANCO_SHOTS")
ok = True

def wait(ms=60):
    QTest.qWait(ms)

def check(cond, what):
    global ok
    print(("✓ " if cond else "✗ ") + what, flush=True)
    ok = ok and bool(cond)

def shot(name):
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        shiboken6.wrapInstance(shiboken6.getCppPointer(w)[0], QQuickWindow).grabWindow().save(os.path.join(SHOTS, name + ".png"))

def items(root=None):
    root = root or w.contentItem()
    out = [root]
    for c in root.childItems():
        out += items(c)
    return out

def shown(it):
    """visibile davvero e DENTRO la finestra (la pagina dell'altra scheda è fuori)"""
    p = it
    while p is not None:
        if not p.isVisible() or p.opacity() <= 0.01:
            return False
        p = p.parentItem()
    if it.width() <= 0 or it.height() <= 0:
        return False
    c = it.mapToScene(QPointF(it.width() / 2, it.height() / 2))
    return 0 <= c.x() <= w.width() and 0 <= c.y() <= w.height()

def find(text):
    hits = [it for it in items() if it.property("text") == text and shown(it)]
    hits.sort(key=lambda i: -i.width() * i.height())
    return hits[0] if hits else None

def center(it):
    p = it.mapToScene(QPointF(it.width() / 2, it.height() / 2))
    return int(p.x()), int(p.y())

def tap_at(x, y):
    QTest.touchEvent(w, dev).press(0, QPoint(x, y), w).commit()
    wait(40)
    QTest.touchEvent(w, dev).release(0, QPoint(x, y), w).commit()
    wait(400)

def tap(text):
    it = find(text)
    if it is None:
        check(False, f"trovo «{text}» sullo schermo")
        return
    tap_at(*center(it))

def swipe(x0, x1, y):
    """uno swipe orizzontale svelto, come il pollice"""
    QTest.touchEvent(w, dev).press(0, QPoint(x0, y), w).commit(); wait(16)
    for k in range(1, 11):
        QTest.touchEvent(w, dev).move(0, QPoint(int(x0 + (x1 - x0) * k / 10), y), w).commit(); wait(16)
    QTest.touchEvent(w, dev).release(0, QPoint(x1, y), w).commit(); wait(700)

def strip():
    for it in items():
        if it.property("labels") is not None and it.property("current") is not None:
            return it
    return None

def labels():
    v = strip().property("labels")
    return list(v.toVariant() if hasattr(v, "toVariant") else v)

def folder_path():
    """il percorso della scheda in vista, letto dalla barra delle briciole
    (PySide non converte il tipo FolderModel del QML)"""
    for it in items():
        if it.property("rootPath") is not None and it.property("path") is not None:
            return it.property("path")
    return None

def tab_center(i):
    s = strip()
    p = s.mapToScene(QPointF(s.width() * (0.25 if i == 0 else 0.75), s.height() * 0.3))
    return int(p.x()), int(p.y())

def content_y():
    for it in items():
        if "ListView" in it.metaObject().className() and (it.property("cacheBuffer") or 0) > 0 and it.property("highlightMoveDuration") == 220:
            return it
    return None

wait(400)
shot("01-avvio")
s = strip()
check(s is not None, "c'è la fascia delle schede")
check(w.property("tab") == 0, "si parte dalla scheda A")
check(labels() == ["Foto", "Memoria interna"], "linguette: A = Foto, B = Memoria interna (%s)" % labels())
check(find("A") is not None and find("B") is not None, "le lettere A e B si vedono")

# A: si entra in Album
tap("Album")
check(folder_path() == FOTO + "/Album", "A: entrato in Album (%s)" % folder_path())
check(labels()[0] == "Album", "la linguetta A dice Album")

# tocco sulla linguetta B
tap_at(*tab_center(1))
wait(300)
check(w.property("tab") == 1, "tocco sulla linguetta B: scheda B in vista")
check(folder_path() == HOME, "B è nella home (%s)" % folder_path())
lv = content_y()
check(lv is not None and lv.property("currentIndex") == 1, "la cartella scorre alla pagina B")
check(len(w.property("back").toVariant() if hasattr(w.property("back"), "toVariant") else w.property("back")) == 0,
      "B ha la sua cronologia (Indietro vuoto)")
shot("02-scheda-b")

tap("Vacanze")
check(folder_path() == HOME + "/Vacanze", "B: entrato in Vacanze")
check(labels() == ["Album", "Vacanze"], "linguette: Album | Vacanze (%s)" % labels())

# swipe verso destra: torna ad A, che è rimasta in Album
mid = int(lv.mapToScene(QPointF(0, lv.height() / 2)).y())
swipe(60, 400, mid)
check(w.property("tab") == 0, "swipe a destra: scheda A")
check(folder_path() == FOTO + "/Album", "A è rimasta in Album")
shot("03-swipe-ad-a")

# swipe verso sinistra: B
swipe(400, 60, mid)
check(w.property("tab") == 1, "swipe a sinistra: scheda B")
check(folder_path() == HOME + "/Vacanze", "B è rimasta in Vacanze")

# Indietro in B riporta alla home, A non si muove
w.goBack(); wait(300)
check(folder_path() == HOME, "Indietro in B: home")
tap_at(*tab_center(0)); wait(300)
check(w.property("tab") == 0 and folder_path() == FOTO + "/Album", "A ancora in Album")
w.goBack(); wait(300)
check(folder_path() == FOTO, "Indietro in A: Foto")

# la cartella di B si ricorda (impostazioni)

# a metà swipe, col dito fermo: si vede il divisore fra le due pagine
QTest.touchEvent(w, dev).press(0, QPoint(380, mid), w).commit(); wait(16)
for k in range(1, 13):
    QTest.touchEvent(w, dev).move(0, QPoint(380 - 15 * k, mid), w).commit(); wait(30)
for _ in range(4):
    QTest.touchEvent(w, dev).move(0, QPoint(200, mid), w).commit(); wait(60)
cx = lv.property("contentX")
check(0 < cx < lv.width(), "a metà swipe le due pagine sono in vista insieme (contentX %d)" % cx)
shot("05-meta-swipe")
QTest.touchEvent(w, dev).release(0, QPoint(200, mid), w).commit(); wait(800)
# SnapOneItem di Qt: al rilascio va alla pagina nella direzione del dito, anche
# sotto metà (verificato 22/11 anche senza divisore: era così già nella 0.9.8-1)
check(w.property("tab") == 1, "lasciato al 40%%: passa a B, nella direzione del dito (tab %s)" % w.property("tab"))

# toccare due volte la stessa linguetta non fa niente di strano
tap_at(*tab_center(0)); tap_at(*tab_center(0))
check(w.property("tab") == 0, "doppio tocco sulla linguetta in vista: resta A")
shot("04-fine")

bad = [m for m in LOG if "TypeError" in m or "ReferenceError" in m or "is not a function" in m or "Binding loop" in m]
check(not bad, "nessun errore JavaScript nel QML (%d)" % len(bad))
for m in bad[:10]:
    print("   ", m)
print("TUTTO OK" if ok else "CI SONO ERRORI")
sys.exit(0 if ok else 1)
