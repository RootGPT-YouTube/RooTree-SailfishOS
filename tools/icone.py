#!/usr/bin/env python3
"""Copia nell'app le icone di Font Awesome Free 5.15.4 (CC BY 4.0) che servono,
con il colore già scritto nell'SVG: niente effetti di colorazione a runtime.
Uso: python3 -I tools/icone.py <cartella svgs/ del pacchetto fontawesome-free-5.15.4>
Il pacchetto: https://registry.npmjs.org/@fortawesome/fontawesome-free/-/fontawesome-free-5.15.4.tgz
(sha256 6d890608585d48f50f6434a90ed3f581e76d360edb26800dba2f782ef3e77e11)."""
import os, re, sys

SRC = sys.argv[1]
R = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(R, "app", "qml", "icons")

UI = "#e6edf3"
# icone dei pulsanti: nome nell'app -> (stile, nome Font Awesome)
PULSANTI = {
    "back": ("solid", "arrow-left"), "forward": ("solid", "arrow-right"), "up": ("solid", "level-up-alt"),
    "menu": ("solid", "ellipsis-v"), "down": ("solid", "chevron-down"), "next": ("solid", "chevron-right"),
    "copy": ("regular", "copy"), "cut": ("solid", "cut"), "paste": ("regular", "clipboard"),
    "delete": ("regular", "trash-alt"), "rename": ("solid", "pen"), "info": ("solid", "info-circle"),
    "newfolder": ("solid", "folder-plus"), "select": ("regular", "check-square"), "check": ("solid", "check"),
    "sort": ("solid", "sort-amount-down"), "hidden": ("regular", "eye-slash"), "shown": ("regular", "eye"),
    "grid": ("solid", "th-large"), "list": ("solid", "list"), "bookmark": ("solid", "bookmark"),
    "close": ("solid", "times"), "home": ("solid", "home"), "refresh": ("solid", "sync-alt"),
    "jobs": ("solid", "tasks"), "warning": ("solid", "exclamation-triangle"), "selectall": ("solid", "check-double"),
    "internal": ("solid", "mobile-alt"), "android": ("brands", "android"), "sdcard": ("solid", "sd-card"),
    "usb": ("brands", "usb"), "disk": ("solid", "hdd"), "cloud": ("solid", "cloud"), "ssh": ("solid", "terminal"),
    "server": ("solid", "server"), "network": ("solid", "network-wired"), "root": ("solid", "hashtag"),
    "lock": ("solid", "lock"), "link": ("solid", "link"), "folder": ("solid", "folder"),
    "share": ("solid", "share-alt"), "openwith": ("solid", "external-link-alt"), "wrap": ("solid", "align-left"),
    "image": ("regular", "image"),
}


def scrivi(stile, nome, colore, dest):
    s = open(os.path.join(SRC, stile, nome + ".svg")).read()
    s = re.sub(r"<!--.*?-->", "", s, flags=re.S)
    s = s.replace("<svg ", '<svg fill="%s" ' % colore, 1)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with open(dest, "w") as f:
        f.write("<!-- Font Awesome Free 5.15.4 by @fontawesome - https://fontawesome.com"
                " License - https://fontawesome.com/license/free (CC BY 4.0) -->\n" + s.strip() + "\n")


# varianti per i pulsanti a disco d'acciaio (come il centro di controllo di
# RooTundra): simbolo inciso scuro, il filo di luce sotto l'incisione, e il
# simbolo acceso quando il pulsante è attivo
VARIANTI = {"": UI, "inciso": "#2d343c", "luce": "#ffffff\" fill-opacity=\"0.35", "acceso": "#d2faff"}
# i glifi incisi sulle icone d'acciaio dei tipi (grafica/tipi.py li disegna):
# bianchi, il colore lo decide l'incisione
GLIFI = {
    "image": ("solid", "image"), "music": ("solid", "music"), "film": ("solid", "film"),
    "code": ("solid", "code"), "table": ("solid", "table"), "chart": ("solid", "chart-bar"),
    "android": ("brands", "android"), "download": ("solid", "download"), "camera": ("solid", "camera"),
    "desktop": ("solid", "desktop"), "documents": ("solid", "file-alt"), "link": ("solid", "link"),
}
for app, (stile, fa) in GLIFI.items():
    scrivi(stile, fa, "#ffffff", os.path.join(R, "grafica", "glifi", app + ".svg"))
for app, (stile, fa) in PULSANTI.items():
    for cartella, colore in VARIANTI.items():
        scrivi(stile, fa, colore, os.path.join(OUT, cartella, app + ".svg"))
print(len(PULSANTI), "pulsanti,", len(GLIFI), "glifi")
