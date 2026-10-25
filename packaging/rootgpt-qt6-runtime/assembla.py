#!/usr/bin/env python3
"""Runtime Qt 6.12 condiviso fra le app Qt6 di RootGPT (rootgpt-qt6-runtime).

Prende lo stack Qt dal target sb2 clonato (…-qt612, costruito dagli script di
RooTitanium/packaging/qt612) e ne tiene solo quello che serve a un'app Qt Quick
(niente motore web): librerie Qt, plugin di piattaforma Wayland, immagini, TLS,
rete, tastiera, moduli QML. Le librerie non Qt che le servono si portano dietro
anch'esse (come fa RooTitanium), tranne quelle di sistema in SISTEMA, che il
pacchetto dichiara nei Requires.

Unico pezzo non preso dal target: il plugin Maliit (tastiera di sistema),
ricompilato per Qt 6.12 da RooTitanium (vedi RooTitanium/packaging/qt612/README.md).

Da lanciare DENTRO il build engine:
  sfdk engine exec -- python3 <repo>/packaging/rootgpt-qt6-runtime/assembla.py --arch aarch64
Scrive <repo>/scratch/qt6-runtime-<arch>/runtime e finisce con RUNTIME-OK
oppure RUNTIME-INCOMPLETO (exit 1).
"""
import argparse, os, shutil, subprocess, sys

ap = argparse.ArgumentParser()
ap.add_argument("--arch", default="aarch64", choices=["aarch64", "armv7hl"])
ap.add_argument("--maliit", help="plugin Maliit per Qt 6.12 (predefinito: quello dello staging di RooTitanium)")
a = ap.parse_args()

R = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
L = "lib64" if a.arch == "aarch64" else "lib"
T = f"SailfishOS-5.1.0.11-{a.arch}-qt612"
TGT = f"/srv/mer/targets/{T}"
QT = f"{TGT}/usr/{L}/qt6"
DST = f"{R}/scratch/qt6-runtime-{a.arch}/runtime"
MALIIT = a.maliit or (f"{os.path.dirname(R)}/RooTitanium/scratch/pkg-staging-qt612-{a.arch}"
                      "/bundle/plugins/platforminputcontexts/libmaliitplatforminputcontextplugin.so")
ENV = dict(os.environ, LC_ALL="C", LANG="C")

# Di serie su Sailfish OS 5.1: il runtime non le porta e lo spec le chiede per
# soname. È l'elenco dei Requires di RooTitanium (letto dai NEEDED il 12/10/2026)
# più glibc. libcrypto sta qui insieme a libssl: le due vanno in coppia.
SISTEMA = {
    "libc.so.6", "libm.so.6", "libdl.so.2", "libpthread.so.0", "librt.so.1", "libutil.so.1", "libresolv.so.2",
    "ld-linux-aarch64.so.1", "ld-linux-armhf.so.3", "libstdc++.so.6", "libgcc_s.so.1",
    "libssl.so.3", "libcrypto.so.3", "libEGL.so.1", "libGLESv2.so.2", "libgbm.so.1", "libdrm.so.2",
    "libwayland-client.so.0", "libwayland-cursor.so.0", "libwayland-egl.so.1",
    "libmtdev.so.1", "libudev.so.1", "libsystemd.so.0", "libsqlite3.so.0",
}

# moduli QML (cartelle di qml/) e plugin (cartelle o file di plugins/)
QML = ["QML", "QtCore", "QtQml", "QtNetwork", "QtQuick",
       "Qt/labs/folderlistmodel", "Qt/labs/settings", "Qt/labs/qmlmodels",
       "Qt/labs/sharedimage", "Qt/labs/animation", "builtins.qmltypes", "jsroot.qmltypes"]
# dentro QtQuick: strumenti di sviluppo e stili di altre piattaforme non servono
QML_VIA = ["QtQuick/tooling", "QtQuick/Particles",
           "QtQuick/Controls/macOS", "QtQuick/Controls/iOS", "QtQuick/Controls/Windows",
           "QtQuick/Controls/FluentWinUI3", "QtQuick/Controls/Imagine"]
PLUGIN = ["platforms/libqwayland.so", "wayland-decoration-client", "wayland-graphics-integration-client",
          "wayland-shell-integration", "imageformats", "iconengines", "tls/libqopensslbackend.so",
          "tls/libqcertonlybackend.so", "networkinformation/libqconnman.so", "sqldrivers",
          "platforminputcontexts/libqtvirtualkeyboardplugin.so"]


def is_elf(p):
    try:
        with open(p, "rb") as f:
            return f.read(4) == b"\x7fELF"
    except OSError:
        return False


def needed(p):
    out = subprocess.run(["readelf", "-d", p], capture_output=True, text=True, env=ENV).stdout
    return [l[l.index("[") + 1:l.index("]")] for l in out.splitlines() if "(NEEDED)" in l]


def elfs(base):
    for dp, dn, fn in os.walk(base):
        for f in fn:
            p = f"{dp}/{f}"
            if not os.path.islink(p) and is_elf(p):
                yield p


def copia(src, dst):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    (shutil.copytree if os.path.isdir(src) else shutil.copy2)(src, dst)


def copia_soname(soname):
    """Il file vero dal target, più il link col soname."""
    reale = os.path.realpath(f"{TGT}/usr/{L}/{soname}")
    if not os.path.isfile(reale):
        return False
    shutil.copy2(reale, f"{DST}/lib/{os.path.basename(reale)}")
    if os.path.basename(reale) != soname:
        os.symlink(os.path.basename(reale), f"{DST}/lib/{soname}")
    return True


mancanti = []
if os.path.exists(DST):
    shutil.rmtree(DST)
os.makedirs(f"{DST}/lib")

# ── 1) moduli QML e plugin ───────────────────────────────────────────────────
for m in QML:
    s = f"{QT}/qml/{m}"
    if os.path.exists(s):
        copia(s, f"{DST}/qml/{m}")
    else:
        mancanti.append(f"qml/{m}")
for m in QML_VIA:
    shutil.rmtree(f"{DST}/qml/{m}", ignore_errors=True)
for p in PLUGIN:
    s = f"{QT}/plugins/{p}"
    if os.path.exists(s):
        copia(s, f"{DST}/plugins/{p}")
    else:
        mancanti.append(f"plugins/{p}")
if os.path.isfile(MALIIT):
    copia(MALIIT, f"{DST}/plugins/platforminputcontexts/libmaliitplatforminputcontextplugin.so")
else:
    mancanti.append(f"plugin Maliit ({MALIIT})")

# ── 2) chiusura sui NEEDED: ogni libreria richiesta che non è di sistema ─────
for giro in range(10):
    presenti = set(os.listdir(f"{DST}/lib"))
    nuove = {n for p in elfs(DST) for n in needed(p)} - presenti - SISTEMA
    if not nuove:
        break
    for n in sorted(nuove):
        if not copia_soname(n):
            mancanti.append(f"lib/{n} (assente nel target)")
            SISTEMA.add(n)          # non riprovarci al giro dopo

# ── 3) strip e permessi (sotto sb2 qualche file del target esce 000) ─────────
tutti = list(elfs(DST))
for i in range(0, len(tutti), 200):
    subprocess.run(["sb2", "-t", T, "strip", "--strip-unneeded"] + tutti[i:i + 200],
                   capture_output=True, env=ENV)
for dp, dn, fn in os.walk(DST):
    for d in dn:
        os.chmod(f"{dp}/{d}", 0o755)
    for f in fn:
        p = f"{dp}/{f}"
        if not os.path.islink(p):
            os.chmod(p, 0o755 if is_elf(p) else 0o644)

# ── 4) verifica ──────────────────────────────────────────────────────────────
presenti = set(os.listdir(f"{DST}/lib"))
non_risolte = {}
for p in elfs(DST):
    for n in needed(p):
        if n not in presenti:
            non_risolte.setdefault(n, []).append(os.path.relpath(p, DST))
estranee = {n: v for n, v in non_risolte.items() if n not in SISTEMA or any(n in m for m in mancanti)}
dim = subprocess.run(["du", "-sh", DST], capture_output=True, text=True).stdout.split()[0]
print(f"runtime: {DST} ({dim}, {len(tutti)} ELF)")
print("di sistema richieste:", " ".join(sorted(n for n in non_risolte if n in SISTEMA)))
for x in mancanti:
    print("   ! manca", x)
for n, v in sorted(estranee.items()):
    print(f"   ! non risolta {n}  <- {', '.join(v[:3])}")
ok = not mancanti and not estranee
print("RUNTIME-OK" if ok else "RUNTIME-INCOMPLETO")
sys.exit(0 if ok else 1)
