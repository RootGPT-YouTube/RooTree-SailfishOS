#!/usr/bin/env python3
# Icona HD di RooTelegram (14/10/2026) per il web (OpenRepos, GitHub): la stessa
# icona «acciaio / tech» di mieapp.py + launcher.py (XPOCOM4PRO4G/hadk/glacier/
# rootundra/grafica), ma ridisegnata a D px invece di ingrandire quella da 256.
# Ogni misura in pixel di launcher.icona() e' moltiplicata per k = D / 256. Il
# disegno inciso resta quello estratto a 384 px come in mieapp (l'utente vuole
# l'icona da 256 identica, non ridisegnata): lo si ingrandisce con bordi netti.
# Uso: python3 icona-hd.py <icona vecchia.png> <uscita.png> [D=1024]
import math, sys
import numpy as np
from PySide6.QtGui import (QGuiApplication, QImage, QPainter, QColor, QLinearGradient,
                           QConicalGradient, QPen)
from PySide6.QtCore import Qt, QPointF, QRectF


def a_numpy(img):
    img = img.convertToFormat(QImage.Format_ARGB32)
    w, h = img.width(), img.height()
    buf = np.frombuffer(img.constBits(), np.uint8, img.sizeInBytes()).reshape(h, img.bytesPerLine())
    return buf[:, :w * 4].reshape(h, w, 4).astype(np.float32)   # B, G, R, A


def disegno(src, lato=384):
    """mieapp.disegno() con numpy: stessa estrazione a 384 px, stessa forma."""
    src = src.convertToFormat(QImage.Format_ARGB32).scaled(lato, lato, Qt.KeepAspectRatio,
                                                           Qt.SmoothTransformation)
    px = a_numpy(src)
    h, w = px.shape[:2]
    c, r = w / 2, min(w, h) / 2
    yy, xx = np.mgrid[0:h, 0:w]
    dentro = (xx - c) ** 2 + (yy - c) ** 2 <= (r * 0.86) ** 2
    bianco = px[..., :3].min(axis=2)
    a = np.clip((bianco - 150) / 80.0, 0, 1) * (px[..., 3] / 255.0)
    a = np.where(dentro & (a > 0.02), np.floor(255 * a), 0)
    pieno = float((a > 128).sum()) / (w * h)
    # stesso ispessimento di mieapp (raggio w/190, cioe' lo stesso in proporzione);
    # in HD su un disco: col quadrato ogni bordo obliquo diventa una scala
    rr = max(1, round(w / 190))
    if w <= 384:
        pad = np.pad(a, rr)
        spesso = np.zeros_like(a)
        for dy in range(-rr, rr + 1):
            for dx in range(-rr, rr + 1):
                spesso = np.maximum(spesso, pad[rr + dy:rr + dy + h, rr + dx:rr + dx + w])
    else:
        from scipy import ndimage
        yk, xk = np.mgrid[-rr:rr + 1, -rr:rr + 1]
        spesso = ndimage.grey_dilation(a, footprint=(xk * xk + yk * yk) <= rr * rr + rr)
    ys, xs = np.nonzero(spesso > 40)
    x0, x1, y0, y1 = xs.min(), xs.max(), ys.min(), ys.max()
    m = round(6 * w / 384)                      # il margine di mieapp, in proporzione
    lato_c = max(x1 - x0, y1 - y0) + m
    cx, cy = (x0 + x1) // 2, (y0 + y1) // 2
    out = np.zeros((h, w, 4), np.uint8)
    out[..., 0:3] = 255
    out[..., 3] = spesso.astype(np.uint8)
    img = QImage(out.tobytes(), w, h, w * 4, QImage.Format_ARGB32).copy()
    return img.copy(cx - lato_c // 2, cy - lato_c // 2, lato_c, lato_c), pieno


SMUSSO = 0.7      # px a 384: provati 0.7 e 1.1, a 1.1 il disegno si impasta (14/10)


def ingrandisci(g, size):
    """Il disegno da 384 px portato a `size` senza cambiarne la forma: alfa
    ingrandito con un filtro morbido, poi una curva di contrasto simmetrica
    attorno a 128 ridà i bordi netti nello stesso punto (niente sfocato)."""
    from PIL import Image
    al = a_numpy(g)[..., 3]
    # i gradini dell'ispessimento a quadrato di mieapp: a 256 non si vedono,
    # ingranditi si'. Una sfocatura minima (a 384 px) li arrotonda senza
    # spostare i bordi, che la curva qui sotto rimette netti.
    from scipy import ndimage
    al = np.clip(ndimage.gaussian_filter(al, sigma=SMUSSO), 0, 255).astype(np.uint8)
    if g.width() >= g.height():
        nw, nh = size, max(1, round(size * g.height() / g.width()))
    else:
        nw, nh = max(1, round(size * g.width() / g.height())), size
    al = np.asarray(Image.fromarray(al).resize((nw, nh), Image.BICUBIC)).astype(np.float32)
    f = nw / float(g.width())                  # quanto si ingrandisce
    mezza = max(24.0, min(128.0, 128.0 / max(1.0, f * 0.75)))
    al = np.clip((al - 128.0) * 128.0 / mezza + 128.0, 0, 255).astype(np.uint8)
    out = np.zeros((nh, nw, 4), np.uint8)
    out[..., 0:3] = 255
    out[..., 3] = al
    return QImage(out.tobytes(), nw, nh, nw * 4, QImage.Format_ARGB32).copy()


def glifo(g, colour, size):
    g = g.scaled(size, size, Qt.KeepAspectRatio, Qt.SmoothTransformation)
    out = QImage(g.size(), QImage.Format_ARGB32_Premultiplied)
    out.fill(Qt.transparent)
    q = QPainter(out)
    q.drawImage(0, 0, g)
    q.setCompositionMode(QPainter.CompositionMode_SourceIn)
    q.fillRect(out.rect(), colour)
    q.end()
    return out


def icona(g, scala, D):
    """launcher.icona() con ogni misura assoluta moltiplicata per k."""
    k = D / 256.0
    img = QImage(D, D, QImage.Format_ARGB32_Premultiplied)
    img.fill(Qt.transparent)
    p = QPainter(img)
    p.setRenderHint(QPainter.Antialiasing)
    p.setRenderHint(QPainter.SmoothPixmapTransform)
    c = D / 2
    r = D / 2 - 2 * k
    ri = r * 0.86
    bz = QLinearGradient(0, c - r, 0, c + r)
    bz.setColorAt(0, QColor(96, 106, 116))
    bz.setColorAt(1, QColor(22, 27, 33))
    p.setPen(Qt.NoPen)
    p.setBrush(bz)
    p.drawEllipse(QPointF(c, c), r, r)
    rr = (r + ri) / 2
    rect = QRectF(c - rr, c - rr, 2 * rr, 2 * rr)
    for w, a in ((r - ri + 6 * k, 60), (r - ri - 2 * k, 130), (max(3 * k, (r - ri) * 0.35), 255)):
        p.setPen(QPen(QColor(0, 115, 255, a), w, Qt.SolidLine, Qt.RoundCap))
        p.drawArc(rect, -60 * 16, -60 * 16)
        p.drawArc(rect, -120 * 16, 60 * 16)
    p.setPen(Qt.NoPen)
    cg = QConicalGradient(c, c, 30)
    for t, v in ((0, 175), (0.12, 232), (0.25, 150), (0.4, 217), (0.55, 140), (0.7, 226), (0.85, 160), (1, 175)):
        cg.setColorAt(t, QColor(v, v + 6, v + 12))
    p.setBrush(cg)
    p.drawEllipse(QPointF(c, c), ri, ri)
    dome = QLinearGradient(0, c - ri, 0, c + ri)
    dome.setColorAt(0, QColor(255, 255, 255, 34))
    dome.setColorAt(0.55, QColor(255, 255, 255, 0))
    dome.setColorAt(1, QColor(0, 10, 20, 60))
    p.setBrush(dome)
    p.drawEllipse(QPointF(c, c), ri, ri)
    for colour, dy in ((QColor(255, 255, 255, 90), 1.6 * k), (QColor(55, 65, 75, 190), 0)):
        p.setPen(QPen(colour, 2.4 * k))
        p.setBrush(colour)
        for n in range(6):
            a = n * math.pi / 3 + 0.52
            x0, y0 = c + math.cos(a) * ri * 0.70, c + math.sin(a) * ri * 0.70 + dy
            x1, y1 = c + math.cos(a) * ri * 0.88, c + math.sin(a) * ri * 0.88 + dy
            p.drawLine(QPointF(x0, y0), QPointF(x1, y1))
            p.drawEllipse(QPointF(x1, y1), 3.6 * k, 3.6 * k)
    p.setPen(Qt.NoPen)
    size = int(D * scala)
    edge = glifo(g, QColor(255, 255, 255, 150), size)
    cut = glifo(g, QColor(32, 44, 58), size)
    glint = glifo(g, QColor(0, 115, 255, 120), size)
    x, y = c - cut.width() / 2, c - cut.height() / 2
    p.drawImage(QPointF(x, y + 3 * k), edge)
    p.drawImage(QPointF(x, y - 1 * k), glint)
    p.drawImage(QPointF(x, y), cut)
    p.setBrush(Qt.NoBrush)
    p.setPen(QPen(QColor(255, 255, 255, 90), 2 * k))
    p.drawEllipse(QPointF(c, c), ri - 1 * k, ri - 1 * k)
    p.end()
    return img


def main():
    app = QGuiApplication(sys.argv)
    src, out = sys.argv[1], sys.argv[2]
    D = int(sys.argv[3]) if len(sys.argv) > 3 else 1024
    sorgente = QImage(src)
    # in HD il disegno si estrae alla misura dell'originale (965 px): a 384 i
    # gradini sono grossi quanto lo spazio fra le piste e nessun filtro li toglie
    g, pieno = disegno(sorgente, max(sorgente.width(), sorgente.height()) if D > 256 else 384)
    scala = 0.58 if pieno < 0.12 else 0.42          # la regola di mieapp
    if int(D * scala) > g.width():                  # in HD il disegno si ingrandisce
        g = ingrandisci(g, int(D * scala))
    icona(g, scala, D).save(out)
    print(out, D, 'inchiostro %.2f' % pieno, 'scala', scala)


if __name__ == '__main__':
    main()
