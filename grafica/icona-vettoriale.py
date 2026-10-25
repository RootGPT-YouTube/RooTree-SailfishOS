#!/usr/bin/env python3
# Icona «acciaio / tech» di RooTree (25/10/2026), nello stile vettoriale di
# RooTheater e RooTelegram: disco, piste incise e arco LED di icona-hd.py
# (copiato da RooTheater), e al centro una cartella a facce scure con le pieghe
# in blu elettrico. Sul pannello davanti le piste del circuito crescono come un
# albero (il «Tree» del nome): un tronco dal bordo in basso che si dirama.
# Ogni misura si disegna a 4x e si riduce con Lanczos: bordi lisci anche a 86 px.
# Uso: QT_QPA_PLATFORM=offscreen python3 icona-vettoriale.py [cartella di uscita=.]
#   scrive <misura>x<misura>/harbour-rootree.png (86..256) e HD/harbour-rootree-{512,1024}.png
import importlib.util, os, sys
from PIL import Image
from PySide6.QtGui import QGuiApplication, QImage, QPainter, QColor, QPen, QPolygonF, QPainterPath
from PySide6.QtCore import Qt, QPointF

QUI = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('hd', os.path.join(QUI, 'icona-hd.py'))
hd = importlib.util.module_from_spec(spec)
spec.loader.exec_module(hd)

# cartella su una griglia di 1000: il pannello dietro con la linguetta, e
# davanti il pannello aperto (un po' inclinato, sporge a destra) diviso in
# labbro chiaro e corpo
DIETRO = [(220, 290), (400, 290), (445, 335), (780, 335), (780, 730), (220, 730)]
LABBRO = [(255, 425), (820, 405), (814, 452), (250, 470)]
CORPO = [(250, 470), (814, 452), (780, 730), (220, 730)]
X0, X1, Y0, Y1 = 220, 820, 290, 730
# l'albero: il tronco nasce dal bordo in basso, ogni ramo finisce in un pallino
TRONCO = (515, 730)
PISTE = [
    [TRONCO, (515, 655), (440, 605), (370, 605)],       # ramo a sinistra
    [(515, 655), (590, 605), (660, 605)],               # ramo a destra
    [(515, 655), (515, 530)],                           # cima
    [(515, 700), (600, 700), (640, 670), (700, 670)],   # ramo basso
]
LARG = 0.50       # larghezza della cartella rispetto al lato dell'icona
INCISO = QColor(32, 44, 58)
BLU = QColor(0, 115, 255, 200)


def icona(D):
    k = D / 256.0
    s = D * LARG / (X1 - X0)
    cx, cy = (X0 + X1) / 2.0, (Y0 + Y1) / 2.0

    def M(p):
        return QPointF(D / 2 + (p[0] - cx) * s, D / 2 + (p[1] - cy) * s + 4 * k)

    vuoto = QImage(1, 1, QImage.Format_ARGB32)
    vuoto.fill(Qt.transparent)
    img = hd.icona(vuoto, 0.5, D)            # disco, piste, arco LED: senza disegno
    p = QPainter(img)
    p.setRenderHint(QPainter.Antialiasing)
    poly = lambda v: QPolygonF([M(q) for q in v])
    facce = [(poly(DIETRO), QColor(24, 33, 44)), (poly(CORPO), QColor(40, 53, 68)),
             (poly(LABBRO), QColor(58, 75, 94))]
    sag = QPainterPath()
    for f, _ in facce:
        sag.addPolygon(f)
    sag = sag.simplified()
    bordo = 3 * k
    # come il disegno inciso di icona-hd: filo bianco sotto, riflesso blu sopra
    for col, dy in ((QColor(255, 255, 255, 150), 3 * k), (QColor(0, 115, 255, 120), -1 * k)):
        p.save()
        p.translate(0, dy)
        p.setPen(QPen(col, bordo, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
        p.setBrush(col)
        p.drawPath(sag)
        p.restore()
    p.setPen(Qt.NoPen)
    for f, c in facce:
        p.setBrush(c)
        p.drawPolygon(f)
    p.setPen(QPen(INCISO, bordo, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
    p.setBrush(Qt.NoBrush)
    p.drawPath(sag)
    # le pieghe in blu: lo spigolo della linguetta, l'orlo e il labbro del pannello davanti
    p.setPen(QPen(BLU, 2.2 * k, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
    for a, b in ((DIETRO[1], DIETRO[2]), (DIETRO[2], DIETRO[3]),
                 (LABBRO[0], LABBRO[1]), (LABBRO[3], LABBRO[2])):
        p.drawLine(M(a), M(b))
    p.setPen(QPen(QColor(150, 175, 200, 230), 2.4 * k, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
    for tr in PISTE:
        pts = [M(q) for q in tr]
        for a, b in zip(pts, pts[1:]):
            p.drawLine(a, b)
        p.setBrush(QColor(150, 175, 200))
        p.drawEllipse(pts[-1], 4 * k, 4 * k)
        p.setBrush(Qt.NoBrush)
    p.end()
    return img


def salva(n, percorso, tmp):
    icona(4 * n).save(tmp)
    os.makedirs(os.path.dirname(percorso), exist_ok=True)
    Image.open(tmp).resize((n, n), Image.LANCZOS).save(percorso, optimize=True)
    print(percorso)


def main():
    app = QGuiApplication(sys.argv)
    out = sys.argv[1] if len(sys.argv) > 1 else QUI
    tmp = os.path.join(out, '.tmp-icona.png')
    for n in (86, 108, 128, 172, 256):
        salva(n, os.path.join(out, '%dx%d' % (n, n), 'harbour-rootree.png'), tmp)
    for n in (512, 1024):
        salva(n, os.path.join(out, 'HD', 'harbour-rootree-%d.png' % n), tmp)
    os.remove(tmp)


if __name__ == '__main__':
    main()
