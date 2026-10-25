#pragma once

#include <QImage>
#include <QQuickAsyncImageProvider>
#include <QThreadPool>

// Le anteprime di immagini, video e PDF per la griglia: «image://thumb/<percorso>»
// dal QML (percorso con encodeURIComponent). Si fanno su un pool di thread e
// si tengono nella cache standard freedesktop (~/.cache/thumbnails), che
// condividono anche le altre app che la seguono.
class ThumbProvider : public QQuickAsyncImageProvider
{
public:
    // full = false: anteprime in cache (image://thumb); true: l'immagine intera
    // per il visore, ridotta a quanto chiede il QML (image://picture), senza cache
    explicit ThumbProvider(bool full = false);
    QQuickImageResponse *requestImageResponse(const QString &id, const QSize &requestedSize) override;

private:
    bool m_full;
    QThreadPool m_pool;
};

namespace thumbnailer {
// l'anteprima di `path` entro `size` px di lato (dalla cache, o fatta ora);
// un'immagine nulla se il tipo non si sa leggere o il file è rotto
QImage thumbnail(const QString &path, int size);
// un'immagine (anche WebP, TIFF, BMP) entro `size` px di lato, ruotata secondo l'EXIF
QImage picture(const QString &path, int size);
}
