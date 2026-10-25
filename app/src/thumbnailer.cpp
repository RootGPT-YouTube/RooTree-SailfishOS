#include "thumbnailer.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFileInfo>
#include <QImageReader>
#include <QMimeDatabase>
#include <QRunnable>
#include <QSaveFile>
#include <QStandardPaths>
#include <QUrl>

#include <memory>
#include <mutex>

#include <gst/app/gstappsink.h>
#include <gst/gst.h>
#include <gst/video/video.h>
#include <poppler/cpp/poppler-document.h>
#include <poppler/cpp/poppler-page-renderer.h>
#include <poppler/cpp/poppler-page.h>

#include "qtrust.h"

namespace {

// ── la cache freedesktop: large (256 px) o normal (128 px), nome = md5 dell'URI,
// valida finché Thumb::MTime coincide con la data del file; i fallimenti in
// fail/rootree/, per non riprovare a ogni giro un video rotto
QString cacheDir(const QString &sub)
{
    const QString base = QStandardPaths::writableLocation(QStandardPaths::GenericCacheLocation) + QStringLiteral("/thumbnails/");
    return base + sub;
}

struct CacheKey {
    QString uri;
    QString name;
    qint64 mtime;
};

CacheKey keyFor(const QFileInfo &fi)
{
    const QString uri = QString::fromLatin1(QUrl::fromLocalFile(fi.absoluteFilePath()).toEncoded());
    const QString name = QString::fromLatin1(QCryptographicHash::hash(uri.toUtf8(), QCryptographicHash::Md5).toHex())
                         + QStringLiteral(".png");
    return {uri, name, fi.lastModified().toSecsSinceEpoch()};
}

QImage cached(const QString &file, qint64 mtime)
{
    QImageReader r(file);
    if (r.text(QStringLiteral("Thumb::MTime")).toLongLong() != mtime)
        return {};
    return r.read();
}

void store(const QString &dir, const CacheKey &k, QImage img)
{
    QDir().mkpath(dir);
    QFile::setPermissions(dir, QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
    img.setText(QStringLiteral("Thumb::URI"), k.uri);
    img.setText(QStringLiteral("Thumb::MTime"), QString::number(k.mtime));
    img.setText(QStringLiteral("Software"), QStringLiteral("RooTree"));
    QSaveFile f(dir + QLatin1Char('/') + k.name);
    if (f.open(QIODevice::WriteOnly) && img.save(&f, "PNG"))
        f.commit();
}

// ── immagini: Qt per jpeg/png/gif/svg (con la rotazione EXIF), il nucleo Rust
// per webp/tiff/bmp, che le immagini Qt del runtime non leggono
QImage fromImage(const QString &path, int size)
{
    QImageReader r(path);
    r.setAutoTransform(true);
    if (r.canRead()) {
        const QSize full = r.size();
        // jpeg si decodifica già ridotto: molto più veloce delle foto da 50 MP
        if (full.isValid() && (full.width() > size || full.height() > size))
            r.setScaledSize(full.scaled(size, size, Qt::KeepAspectRatio));
        QImage img = r.read();
        if (!img.isNull())
            return img.width() > size || img.height() > size ? img.scaled(size, size, Qt::KeepAspectRatio, Qt::SmoothTransformation) : img;
    }
    try {
        const auto p = rootree::decode_picture(qtrust::str(path), uint32_t(size));
        return QImage(p.rgba.data(), int(p.width), int(p.height), int(p.width) * 4, QImage::Format_RGBA8888).copy();
    } catch (const rust::Error &) {
        return {};
    }
}

// ── video: un fotogramma al 10% con GStreamer (quello di sistema), in software:
// i decodificatori hardware droid danno buffer che videoconvert non legge
void initGst()
{
    static std::once_flag once;
    std::call_once(once, [] {
        gst_init(nullptr, nullptr);
        GstRegistry *reg = gst_registry_get();
        GList *features = gst_registry_get_feature_list(reg, GST_TYPE_ELEMENT_FACTORY);
        for (GList *l = features; l; l = l->next) {
            GstPluginFeature *f = GST_PLUGIN_FEATURE(l->data);
            if (g_str_has_prefix(gst_plugin_feature_get_name(f), "droid"))
                gst_plugin_feature_set_rank(f, GST_RANK_NONE);
        }
        gst_plugin_feature_list_free(features);
    });
}

QImage fromVideo(const QString &path, int size)
{
    initGst();
    GstElement *play = gst_element_factory_make("playbin", nullptr);
    if (!play)
        return {};
    const QByteArray desc = QStringLiteral("videoconvert ! videoscale ! video/x-raw,format=RGBA,width=%1,pixel-aspect-ratio=1/1 ! appsink name=sink sync=false")
                                .arg(size).toUtf8();
    GstElement *bin = gst_parse_bin_from_description(desc.constData(), TRUE, nullptr);
    GstElement *noAudio = gst_element_factory_make("fakesink", nullptr);
    const QByteArray uri = QUrl::fromLocalFile(path).toEncoded();
    // flags = 1: solo il video (niente audio, niente sottotitoli)
    g_object_set(play, "uri", uri.constData(), "video-sink", bin, "audio-sink", noAudio, "flags", 1, nullptr);
    QImage out;
    auto wait = [play] { return gst_element_get_state(play, nullptr, nullptr, 6 * GST_SECOND) == GST_STATE_CHANGE_SUCCESS; };
    if (gst_element_set_state(play, GST_STATE_PAUSED) != GST_STATE_CHANGE_FAILURE && wait()) {
        gint64 dur = 0;
        if (gst_element_query_duration(play, GST_FORMAT_TIME, &dur) && dur > 0) {
            gst_element_seek_simple(play, GST_FORMAT_TIME, GstSeekFlags(GST_SEEK_FLAG_FLUSH | GST_SEEK_FLAG_KEY_UNIT), dur / 10);
            wait();
        }
        GstElement *sink = gst_bin_get_by_name(GST_BIN(bin), "sink");
        GstSample *sample = sink ? gst_app_sink_try_pull_preroll(GST_APP_SINK(sink), 4 * GST_SECOND) : nullptr;
        if (sample) {
            GstVideoInfo info;
            GstBuffer *buf = gst_sample_get_buffer(sample);
            GstMapInfo map;
            if (gst_video_info_from_caps(&info, gst_sample_get_caps(sample)) && gst_buffer_map(buf, &map, GST_MAP_READ)) {
                out = QImage(map.data, GST_VIDEO_INFO_WIDTH(&info), GST_VIDEO_INFO_HEIGHT(&info),
                             GST_VIDEO_INFO_PLANE_STRIDE(&info, 0), QImage::Format_RGBA8888).copy();
                gst_buffer_unmap(buf, &map);
            }
            gst_sample_unref(sample);
        }
        if (sink)
            gst_object_unref(sink);
    }
    gst_element_set_state(play, GST_STATE_NULL);
    gst_object_unref(play);
    if (out.height() > size)
        out = out.scaled(size, size, Qt::KeepAspectRatio, Qt::SmoothTransformation);
    return out;
}

// ── PDF: la prima pagina con poppler (quello di sistema, lo usa sailfish-office)
QImage fromPdf(const QString &path, int size)
{
    std::unique_ptr<poppler::document> doc(poppler::document::load_from_file(QFile::encodeName(path).toStdString()));
    if (!doc || doc->is_locked() || doc->pages() < 1)
        return {};
    std::unique_ptr<poppler::page> page(doc->create_page(0));
    if (!page)
        return {};
    const poppler::rectf r = page->page_rect();
    const double side = std::max(r.width(), r.height());
    if (side <= 0)
        return {};
    const double dpi = 72.0 * size / side;
    poppler::page_renderer renderer;
    renderer.set_render_hints(poppler::page_renderer::antialiasing | poppler::page_renderer::text_antialiasing);
    const poppler::image img = renderer.render_page(page.get(), dpi, dpi);
    if (!img.is_valid())
        return {};
    return QImage(reinterpret_cast<const uchar *>(img.const_data()), img.width(), img.height(), img.bytes_per_row(),
                  QImage::Format_ARGB32).copy();
}

// il lavoro sul pool, separato dalla risposta: se il QML rinuncia (si è
// scorsa la griglia) la risposta sparisce, il segnale si scollega da solo e il
// thread non scrive su un oggetto distrutto (lo schema dell'esempio di Qt)
class ThumbJob : public QObject, public QRunnable
{
    Q_OBJECT
public:
    ThumbJob(const QString &path, int size, bool full)
        : m_path(path), m_size(size), m_full(full)
    {
    }
    void run() override { emit done(m_full ? thumbnailer::picture(m_path, m_size) : thumbnailer::thumbnail(m_path, m_size)); }
signals:
    void done(const QImage &image);

private:
    QString m_path;
    int m_size;
    bool m_full;
};

class ThumbResponse : public QQuickImageResponse
{
public:
    ThumbResponse(const QString &path, int size, bool full, QThreadPool *pool)
    {
        auto *job = new ThumbJob(path, size, full);
        connect(job, &ThumbJob::done, this, [this](const QImage &img) {
            m_image = img;
            emit finished();
        });
        pool->start(job);
    }
    QQuickTextureFactory *textureFactory() const override
    {
        return QQuickTextureFactory::textureFactoryForImage(m_image);
    }
    QString errorString() const override
    {
        return m_image.isNull() ? QStringLiteral("no thumbnail") : QString();
    }

private:
    QImage m_image;
};

} // namespace

QImage thumbnailer::thumbnail(const QString &path, int size)
{
    const QFileInfo fi(path);
    if (!fi.isFile())
        return {};
    const QString sub = size > 128 ? QStringLiteral("large") : QStringLiteral("normal");
    const int side = size > 128 ? 256 : 128;
    const CacheKey key = keyFor(fi);
    QImage img = cached(cacheDir(sub) + QLatin1Char('/') + key.name, key.mtime);
    if (!img.isNull())
        return img;
    if (!cached(cacheDir(QStringLiteral("fail/rootree")) + QLatin1Char('/') + key.name, key.mtime).isNull())
        return {};

    const QString mime = QMimeDatabase().mimeTypeForFile(fi).name();
    if (mime.startsWith(QLatin1String("image/")))
        img = fromImage(path, side);
    else if (mime.startsWith(QLatin1String("video/")))
        img = fromVideo(path, side);
    else if (mime == QLatin1String("application/pdf"))
        img = fromPdf(path, side);
    else
        return {};

    if (img.isNull()) {
        // il segno del fallimento: un PNG di 1×1 con la data del file
        QImage mark(1, 1, QImage::Format_ARGB32);
        mark.fill(Qt::transparent);
        store(cacheDir(QStringLiteral("fail/rootree")), key, mark);
        return {};
    }
    store(cacheDir(sub), key, img);
    return img;
}

QImage thumbnailer::picture(const QString &path, int size)
{
    return fromImage(path, size);
}

ThumbProvider::ThumbProvider(bool full)
    : m_full(full)
{
    // due alla volta: un telefono ha pochi core e la decodifica video pesa
    m_pool.setMaxThreadCount(2);
}

QQuickImageResponse *ThumbProvider::requestImageResponse(const QString &id, const QSize &requestedSize)
{
    const int asked = std::max(requestedSize.width(), requestedSize.height());
    const int size = m_full ? (asked > 0 ? asked : 4096) : (asked > 128 ? 256 : 128);
    return new ThumbResponse(QUrl::fromPercentEncoding(id.toUtf8()), size, m_full, &m_pool);
}

#include "thumbnailer.moc"
