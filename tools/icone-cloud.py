#!/usr/bin/env python3
"""Nuvole con la lettera dei servizi (M = Mega, K = kDrive): la nuvola di
Font Awesome Free 5.15.4 (CC BY 4.0) con la lettera ritagliata dentro
(fill-rule evenodd), nelle quattro varianti dei pulsanti. Lettere disegnate
per RooTree: non sono i loghi dei servizi.
Uso: python3 -I tools/icone-cloud.py"""
import os

R = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(R, "app", "qml", "icons")
NUVOLA = ("M537.6 226.6c4.1-10.7 6.4-22.4 6.4-34.6 0-53-43-96-96-96-19.7 0-38.1 6-53.3 16.2C367 64.2 315.3 32 256 32"
          "c-88.4 0-160 71.6-160 160 0 2.7.1 5.4.2 8.1C40.2 219.8 0 273.2 0 336c0 79.5 64.5 144 144 144h368"
          "c70.7 0 128-57.3 128-128 0-61.9-44-113.6-102.4-125.4z")
LETTERE = {
    "mega": "M212 440V220h54l54 98 54-98h54v220h-48V310l-46 80h-28l-46-80v130z",
    "kdrive": "M226 220h50v88l78-88h62l-90 98 94 122h-62l-66-88-16 17v71h-50z",
}
VARIANTI = {"": "#e6edf3", "inciso": "#2d343c", "luce": "#ffffff\" fill-opacity=\"0.35", "acceso": "#d2faff"}
for nome, lettera in LETTERE.items():
    for cartella, colore in VARIANTI.items():
        with open(os.path.join(OUT, cartella, nome + ".svg"), "w") as f:
            f.write("<!-- Nuvola: Font Awesome Free 5.15.4 by @fontawesome - https://fontawesome.com"
                    " License - https://fontawesome.com/license/free (CC BY 4.0); lettera disegnata per RooTree"
                    " (non è il logo del servizio) -->\n"
                    '<svg fill="%s" fill-rule="evenodd" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 512">'
                    '<path d="%s%s"/></svg>\n' % (colore, NUVOLA, lettera))
