import sys, os
from PySide6.QtCore import QUrl, QPoint, QMetaObject, Q_RETURN_ARG, Q_ARG, Qt, QTimer
from PySide6.QtGui import QGuiApplication, QInputMethodEvent
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtTest import QTest

B = os.path.dirname(os.path.abspath(__file__))
from PySide6.QtCore import qInstallMessageHandler
def _h(mode, ctx, msg):
    if "DBG" in msg or "rror" in msg or "arning" in msg.lower():
        print("LOG", msg, flush=True)
qInstallMessageHandler(_h)
app = QGuiApplication(sys.argv)
eng = QQmlApplicationEngine()
eng.addImportPath(os.path.join(B, "stub"))
# il testo del file: lungo, per poter scorrere
riga = "uno due tre quattro cinque sei sette otto nove dieci"
testo = "\n".join(f"{n:02d} {riga}" for n in range(60)) + "\nultima Vhfg"
eng.load(QUrl.fromLocalFile(os.path.join(B, "harness.qml")))
if not eng.rootObjects():
    sys.exit("QML non caricato")
w = eng.rootObjects()[0]
dev = QTest.createTouchDevice()

def call(name, *args):
    ret = QMetaObject.invokeMethod(w, name, Q_RETURN_ARG("QVariant"), *[Q_ARG("QVariant", a) for a in args])
    return ret.toVariant() if hasattr(ret, "toVariant") else ret

def wait(ms=50):
    QTest.qWait(ms)

def st():
    return call("state")

def tap(x, y, hold=0):
    p = QPoint(int(x), int(y))
    QTest.touchEvent(w, dev).press(0, p, w).commit()
    wait(hold or 40)
    QTest.touchEvent(w, dev).release(0, p, w).commit()
    wait(60)

def tap_jitter(x, y):
    # il dito trema di 3 px: deve restare un tocco
    p = QPoint(int(x), int(y))
    QTest.touchEvent(w, dev).press(0, p, w).commit(); wait(20)
    QTest.touchEvent(w, dev).move(0, QPoint(int(x) + 3, int(y) + 2), w).commit(); wait(20)
    QTest.touchEvent(w, dev).release(0, QPoint(int(x) + 3, int(y) + 2), w).commit(); wait(60)

def drag(x, y, dy):
    QTest.touchEvent(w, dev).press(0, QPoint(int(x), int(y)), w).commit(); wait(16)
    for k in range(1, 21):
        QTest.touchEvent(w, dev).move(0, QPoint(int(x), int(y + dy * k / 20)), w).commit(); wait(16)
    QTest.touchEvent(w, dev).release(0, QPoint(int(x), int(y + dy)), w).commit(); wait(400)

def pt(i):
    p = call("pointOf", i)
    return p["x"], p["y"]

ok = True
def type_text(t):
    # come la tastiera di Sailfish: testo confermato dal metodo d'immissione
    ev = QInputMethodEvent("", [])
    ev.setCommitString(t)
    QGuiApplication.sendEvent(QGuiApplication.focusObject(), ev)
    wait(60)
def check(cond, what):
    global ok
    print(("OK   " if cond else "NO   ") + what)
    ok = ok and cond

from PySide6.QtCore import QObject
viewer = w.findChild(QObject, "viewer")
# Opener.readText legge da qui
from PySide6.QtQml import QQmlComponent
op = eng.singletonInstance("RooTree", "Opener")
op.setProperty("text", testo)
QMetaObject.invokeMethod(viewer, "openFor", Q_ARG("QVariant", "/tmp/x.txt"))
wait(300)
print("MISURE", {k: v for k, v in st().items() if k != "text"})

# --- in lettura
i = testo.index("cinque")
x, y = pt(i + 2)
tap(x, y, hold=900)
s = st(); check(s["sel"] == "cinque", f"lettura: pressione lunga seleziona la parola ({s['sel']!r})")
x2, y2 = pt(testo.index("otto"))
print("PUNTO otto", x2, y2, "cinque", x, y)
tap(x2, y2)
s = st(); check(s["sel"] == "", "lettura: un tocco fuori toglie la selezione")
tap(x, y); tap(x, y)  # doppio tocco
s = st(); check(s["sel"] == "cinque", f"lettura: doppio tocco seleziona la parola ({s['sel']!r})")

# trascinare la maniglia di fine una riga sotto
def drag_from(x, y, dx, dy, hold=0):
    QTest.touchEvent(w, dev).press(0, QPoint(int(x), int(y)), w).commit(); wait(hold or 30)
    for k in range(1, 16):
        QTest.touchEvent(w, dev).move(0, QPoint(int(x + dx * k / 15), int(y + dy * k / 15)), w).commit(); wait(16)
    QTest.touchEvent(w, dev).release(0, QPoint(int(x + dx), int(y + dy)), w).commit(); wait(150)

def handle_test(label):
    wait(700)
    tap(x, y); tap(x, y)  # «cinque»
    h = call("handleOf", True)
    print("PRIMA maniglia", label, h, repr(st()["sel"]), "contentY", st()["contentY"], "fondo flick", st()["flickBottom"])
    drag_from(h["x"], h["y"], 0, h["line"] * 1.0)
    s = st(); check("\n" in s["sel"] or len(s["sel"]) > 12, f"{label}: maniglia trascinata alla riga sotto allarga la selezione ({s['sel']!r})")
    wait(700)
    tap(x2, y2)
    # pressione lunga e poi giù senza alzare il dito
    QTest.touchEvent(w, dev).press(0, QPoint(int(x), int(y)), w).commit(); wait(900)
    line = call("handleOf", True)["line"]
    for k in range(1, 16):
        QTest.touchEvent(w, dev).move(0, QPoint(int(x), int(y + line * 1.0 * k / 15)), w).commit(); wait(16)
    QTest.touchEvent(w, dev).release(0, QPoint(int(x), int(y + line)), w).commit(); wait(150)
    s = st(); check(len(s["sel"]) > 12, f"{label}: pressione lunga e giù col dito allarga la selezione ({s['sel']!r})")
    wait(700)
    tap(x2, y2)

handle_test("lettura")

# --- modifica
viewer.setProperty("editing", True); wait(800)
print("DOPO editing:", {k: v for k, v in st().items() if k != "text"})
tap(x2, y2)
s = st(); check(s["sel"] == "" and abs(s["cursor"] - testo.index("otto")) <= 1 and s["focus"], f"modifica: tocco mette il cursore lì e toglie la selezione (cursore {s['cursor']}, atteso {testo.index('otto')}, fuoco {s['focus']})")
type_text("XY")
s = st(); check("XYotto" in s["text"] or "oXYtto" in s["text"], "modifica: si scrive dove c'è il cursore")
j = testo.index("sette", 60)
x3, y3 = pt(j + 2)
print("PUNTO sette", x3, y3)
tap_jitter(x3, y3)
s = st(); check(abs(s["cursor"] - (j + 2 + 2)) <= 3, f"modifica: tocco col dito che trema sposta lo stesso il cursore ({s['cursor']} ~ {j + 4})")
wait(700)
tap(x3, y3); tap(x3, y3)
s = st(); check(s["sel"] == "sette", f"modifica: doppio tocco seleziona la parola ({s['sel']!r})")
tap(x2, y2)
s = st(); check(s["sel"] == "", "modifica: tocco fuori toglie la selezione")
xs, ys = x, y
x, y = x3, y3
handle_test("modifica")
x, y = xs, ys
# scorrere non sposta il cursore
c0 = st()["cursor"]; y0 = st()["contentY"]
drag(200, 500, -300)
s = st(); check(s["contentY"] > y0 and s["cursor"] == c0, f"modifica: trascinare scorre ({y0}→{s['contentY']}) senza spostare il cursore")
# l'ultima parola, in fondo: si scorre giù e si tocca la fine
drag(200, 450, -5000); drag(200, 450, -5000)
wait(3000)
print("contentY dopo lo scorrimento:", st()["contentY"])
k = len(st()["text"]) - 1
xe, ye = pt(k)
print("PUNTO fine", xe, ye)
tap(xe + 4, ye)
s = st(); check(s["cursor"] >= len(s["text"]) - 1, f"modifica: tocco sull'ultima parola in fondo ({s['cursor']}/{len(s['text'])})")
type_text("Z")
check(st()["text"].rstrip().endswith("Z") or "VhfZg" in st()["text"] or "VhfgZ" in st()["text"], "modifica: si scrive in fondo")
print("TUTTO OK" if ok else "QUALCOSA NON VA")
