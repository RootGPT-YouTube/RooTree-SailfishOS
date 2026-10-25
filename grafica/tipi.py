#!/usr/bin/env python3
# Le icone dei tipi di file di RooTree (25/10/2026): le sagome classiche di un
# file manager, come Dolphin (la cartella con la linguetta, il foglio con
# l'angolo piegato), ma d'acciaio spazzolato nello stile di RooTundra: smusso
# chiaro in alto, ombra in basso, simbolo inciso (scuro con un filo di luce
# sotto) e un LED blu elettrico. Le cartelle hanno il LED sul bordo del
# pannello davanti (come l'icona dell'app); i file una banda LED del colore
# del tipo (verde le immagini, viola l'audio…).
# L'utente, 25/10: «icone in stile RooTundra non intendevo icone rotonde.
# Intendevo le classiche icone da file browser ma in acciaio».
# Ogni icona si disegna a 4x e si riduce con Lanczos.
# Uso: QT_QPA_PLATFORM=offscreen python3 tipi.py → app/qml/icons/tipi/<tipo>.png
import os, sys
import numpy as np
from PIL import Image
from PySide6.QtGui import (QGuiApplication, QImage, QPainter, QPainterPath, QColor, QPen,
                           QLinearGradient, QFont)
from PySide6.QtSvg import QSvgRenderer
from PySide6.QtCore import Qt, QRectF, QPointF

QUI = os.path.dirname(os.path.abspath(__file__))
GLIFI = os.path.join(QUI, 'glifi')
OUT = os.path.join(os.path.dirname(QUI), 'app', 'qml', 'icons', 'tipi')
LATO = 160
D = 4 * LATO
INCISO = QColor(45, 52, 60)
LUCE = QColor(255, 255, 255, 110)
BLU = (0, 115, 255)

# cartelle: nome → glifo inciso sul davanti (None = cartella semplice)
CARTELLE = {
    'folder': None, 'folder-documents': 'documents', 'folder-download': 'download',
    'folder-pictures': 'image', 'folder-camera': 'camera', 'folder-music': 'music',
    'folder-videos': 'film', 'folder-desktop': 'desktop', 'folder-android': 'android',
}
# file: nome → (glifo o testo inciso, colore del LED)
FILE = {
    'file': (None, (120, 135, 150)), 'text': ('#righe', (170, 185, 200)),
    'image': ('image', (40, 210, 110)), 'audio': ('music', (170, 90, 255)),
    'video': ('film', (255, 60, 60)), 'pdf': ('PDF', (255, 70, 40)),
    'archive': ('#zip', (255, 180, 0)), 'code': ('code', (0, 200, 240)),
    'document': ('#righe', (60, 140, 255)), 'spreadsheet': ('table', (40, 200, 90)),
    'presentation': ('chart', (255, 130, 40)), 'package': ('android', (120, 220, 60)),
}


def spazzolato(w, h, chiaro, scuro, seme, verticale=False):
    """Acciaio spazzolato: un gradiente con righe sottili casuali lungo la
    spazzolatura (orizzontale per la cartella, verticale per il foglio)."""
    rnd = np.random.default_rng(seme)
    n = w if verticale else h
    righe = rnd.normal(0, 1, n)
    # righe che si fondono un po' fra loro, come una spazzolatura vera
    k = np.exp(-np.linspace(-2, 2, 9) ** 2)
    righe = np.convolve(righe, k / k.sum(), mode='same') * 9 + rnd.normal(0, 2.5, n)
    t = np.linspace(0, 1, h)[:, None]
    base = chiaro + (scuro - chiaro) * t                     # più chiaro in alto
    base = np.repeat(base, w, axis=1)
    base = base + (righe[None, :] if verticale else righe[:, None])
    # un riflesso largo in diagonale
    yy, xx = np.mgrid[0:h, 0:w]
    base += 18 * np.exp(-(((xx / w) - (yy / h) * 0.6 - 0.25) / 0.18) ** 2)
    v = np.clip(base, 0, 255)
    rgb = np.stack([v, v + 6, v + 12], axis=-1)
    return np.clip(rgb, 0, 255).astype(np.uint8)


def da_numpy(rgb, maschera):
    a = np.dstack([rgb, maschera])
    # QImage ARGB32 su little endian = BGRA in memoria
    bgra = a[..., [2, 1, 0, 3]].copy()
    return QImage(bgra.tobytes(), a.shape[1], a.shape[0], a.shape[1] * 4, QImage.Format_ARGB32).copy()


def maschera(path):
    img = QImage(D, D, QImage.Format_Grayscale8)
    img.fill(0)
    p = QPainter(img)
    p.setRenderHint(QPainter.Antialiasing)
    p.fillPath(path, QColor(255, 255, 255))
    p.end()
    ptr = img.constBits()
    return np.frombuffer(ptr, np.uint8).reshape(D, img.bytesPerLine())[:, :D].copy()


def lastra(p, path, chiaro, scuro, seme, verticale=False):
    """Riempie `path` d'acciaio spazzolato, con lo smusso chiaro in alto e il
    bordo scuro."""
    m = maschera(path)
    p.drawImage(0, 0, da_numpy(spazzolato(D, D, chiaro, scuro, seme, verticale), m))
    p.save()
    p.setClipPath(path)
    sm = QLinearGradient(0, path.boundingRect().top(), 0, path.boundingRect().top() + D * 0.05)
    sm.setColorAt(0, QColor(255, 255, 255, 120))
    sm.setColorAt(1, QColor(255, 255, 255, 0))
    p.setPen(QPen(sm, D * 0.016))
    p.drawPath(path)
    p.restore()
    p.setPen(QPen(QColor(20, 26, 32, 200), D * 0.006))
    p.setBrush(Qt.NoBrush)
    p.drawPath(path)


def glifo(nome, lato):
    img = QImage(lato, lato, QImage.Format_ARGB32_Premultiplied)
    img.fill(Qt.transparent)
    r = QSvgRenderer(os.path.join(GLIFI, nome + '.svg'))
    vb = r.viewBoxF()
    s = lato / max(vb.width(), vb.height())
    w, h = vb.width() * s, vb.height() * s
    p = QPainter(img)
    r.render(p, QRectF((lato - w) / 2, (lato - h) / 2, w, h))
    p.end()
    return img


def tinto(img, colore):
    out = QImage(img.size(), QImage.Format_ARGB32_Premultiplied)
    out.fill(Qt.transparent)
    q = QPainter(out)
    q.drawImage(0, 0, img)
    q.setCompositionMode(QPainter.CompositionMode_SourceIn)
    q.fillRect(out.rect(), colore)
    q.end()
    return out


def incidi(p, img, cx, cy):
    """Il simbolo inciso: filo di luce sotto, poi il solco scuro."""
    x, y = cx - img.width() / 2, cy - img.height() / 2
    p.drawImage(QPointF(x, y + D * 0.006), tinto(img, LUCE))
    p.drawImage(QPointF(x, y), tinto(img, INCISO))


def testo(t, lato):
    img = QImage(lato * 2, lato, QImage.Format_ARGB32_Premultiplied)
    img.fill(Qt.transparent)
    p = QPainter(img)
    p.setRenderHint(QPainter.Antialiasing)
    f = QFont('DejaVu Sans')
    f.setBold(True)
    f.setPixelSize(int(lato * 0.62))
    p.setFont(f)
    p.setPen(QColor(255, 255, 255))
    p.drawText(QRectF(0, 0, lato * 2, lato), Qt.AlignCenter, t)
    p.end()
    return img


def led(p, path, colore, spessore):
    """Una riga LED con il suo alone."""
    for w, a in ((spessore * 3.2, 50), (spessore * 1.8, 110), (spessore, 255)):
        p.setPen(QPen(QColor(*colore, a), w, Qt.SolidLine, Qt.RoundCap))
        p.drawPath(path)


def cartella(simbolo):
    img = QImage(D, D, QImage.Format_ARGB32_Premultiplied)
    img.fill(Qt.transparent)
    p = QPainter(img)
    p.setRenderHint(QPainter.Antialiasing)
    u = D / 1000.0
    # dietro: con la linguetta, acciaio più scuro
    dietro = QPainterPath()
    dietro.moveTo(70 * u, 230 * u)
    dietro.quadTo(70 * u, 170 * u, 130 * u, 170 * u)
    dietro.lineTo(370 * u, 170 * u)
    dietro.quadTo(400 * u, 170 * u, 420 * u, 195 * u)
    dietro.lineTo(460 * u, 245 * u)
    dietro.lineTo(870 * u, 245 * u)
    dietro.quadTo(930 * u, 245 * u, 930 * u, 305 * u)
    dietro.lineTo(930 * u, 780 * u)
    dietro.lineTo(70 * u, 780 * u)
    dietro.closeSubpath()
    lastra(p, dietro, 150, 95, 1)
    # davanti: il pannello grande, acciaio chiaro
    davanti = QPainterPath()
    davanti.addRoundedRect(QRectF(50 * u, 330 * u, 900 * u, 520 * u), 46 * u, 46 * u)
    # un'ombra sotto il pannello davanti
    p.fillPath(davanti.translated(0, 14 * u), QColor(0, 0, 0, 70))
    lastra(p, davanti, 225, 160, 2)
    # il LED blu sul bordo alto del pannello davanti
    orlo = QPainterPath()
    orlo.moveTo(110 * u, 345 * u)
    orlo.lineTo(890 * u, 345 * u)
    led(p, orlo, BLU, 9 * u)
    if simbolo:
        incidi(p, glifo(simbolo, int(300 * u)), 500 * u, 600 * u)
    p.end()
    return img


def foglio(simbolo, colore):
    img = QImage(D, D, QImage.Format_ARGB32_Premultiplied)
    img.fill(Qt.transparent)
    p = QPainter(img)
    p.setRenderHint(QPainter.Antialiasing)
    u = D / 1000.0
    x0, x1, y0, y1, piega = 190 * u, 810 * u, 60 * u, 940 * u, 190 * u
    pagina = QPainterPath()
    pagina.moveTo(x0 + 30 * u, y0)
    pagina.lineTo(x1 - piega, y0)
    pagina.lineTo(x1, y0 + piega)
    pagina.lineTo(x1, y1 - 30 * u)
    pagina.quadTo(x1, y1, x1 - 30 * u, y1)
    pagina.lineTo(x0 + 30 * u, y1)
    pagina.quadTo(x0, y1, x0, y1 - 30 * u)
    pagina.lineTo(x0, y0 + 30 * u)
    pagina.quadTo(x0, y0, x0 + 30 * u, y0)
    pagina.closeSubpath()
    p.fillPath(pagina.translated(10 * u, 14 * u), QColor(0, 0, 0, 70))
    lastra(p, pagina, 225, 165, 3, verticale=True)
    # l'angolo piegato: acciaio più scuro, con la sua ombra
    angolo = QPainterPath()
    angolo.moveTo(x1 - piega, y0)
    angolo.lineTo(x1 - piega, y0 + piega - 20 * u)
    angolo.quadTo(x1 - piega, y0 + piega, x1 - piega + 20 * u, y0 + piega)
    angolo.lineTo(x1, y0 + piega)
    angolo.closeSubpath()
    lastra(p, angolo, 175, 120, 4)
    # la banda LED del tipo, in basso
    banda = QPainterPath()
    banda.moveTo(x0 + 80 * u, y1 - 95 * u)
    banda.lineTo(x1 - 80 * u, y1 - 95 * u)
    led(p, banda, colore, 16 * u)
    cx, cy = (x0 + x1) / 2, 480 * u
    if simbolo == '#righe':
        p.setPen(Qt.NoPen)
        for i, w in enumerate((380, 380, 300, 380, 240)):
            for col, dy in ((LUCE, 5 * u), (INCISO, 0)):
                p.setBrush(col)
                p.drawRoundedRect(QRectF(cx - 190 * u, 300 * u + i * 85 * u + dy, w * u, 34 * u), 14 * u, 14 * u)
    elif simbolo == '#zip':
        # la cerniera degli archivi, al centro del foglio
        p.setPen(Qt.NoPen)
        for i in range(8):
            for col, dy in ((LUCE, 5 * u), (INCISO, 0)):
                p.setBrush(col)
                dx = -40 * u if i % 2 == 0 else 0
                p.drawRoundedRect(QRectF(cx + dx, 120 * u + i * 62 * u + dy, 40 * u, 40 * u), 6 * u, 6 * u)
        for col, dy in ((LUCE, 5 * u), (INCISO, 0)):
            p.setBrush(col)
            p.drawRoundedRect(QRectF(cx - 70 * u, 640 * u + dy, 140 * u, 120 * u), 24 * u, 24 * u)
    elif simbolo and simbolo.isupper():
        incidi(p, testo(simbolo, int(260 * u)), cx, cy)
    elif simbolo:
        incidi(p, glifo(simbolo, int(330 * u)), cx, cy)
    p.end()
    return img


def salva(img, nome):
    tmp = os.path.join(OUT, '.tmp.png')
    img.save(tmp)
    Image.open(tmp).resize((LATO, LATO), Image.LANCZOS).save(os.path.join(OUT, nome + '.png'), optimize=True)
    os.remove(tmp)
    print(nome)


def main():
    app = QGuiApplication(sys.argv)
    os.makedirs(OUT, exist_ok=True)
    for nome, simbolo in CARTELLE.items():
        salva(cartella(simbolo), nome)
    for nome, (simbolo, colore) in FILE.items():
        salva(foglio(simbolo, colore), nome)


if __name__ == '__main__':
    main()
