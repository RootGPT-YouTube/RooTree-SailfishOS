#pragma once

// Conversioni fra i tipi di Qt e quelli del ponte cxx verso il nucleo Rust.

#include <QString>
#include <QStringList>

// rust::Error (le funzioni che rendono Result) non è nell'estratto di cxx.h
// che il ponte generato si porta dentro: serve l'intestazione intera
#include "rust/cxx.h"
#include "rootree_core.h"

namespace qtrust {

// la stringa Rust prende in prestito i byte: il QByteArray deve vivere fino
// alla fine della chiamata, quindi str() si usa solo dentro l'espressione
struct Str {
    QByteArray utf8;
    operator rust::Str() const { return rust::Str(utf8.constData(), size_t(utf8.size())); }
};

inline Str str(const QString &s)
{
    return {s.toUtf8()};
}

inline QString qs(const rust::String &s)
{
    return QString::fromUtf8(s.data(), qsizetype(s.size()));
}

inline rust::Vec<rust::String> vec(const QStringList &list)
{
    rust::Vec<rust::String> v;
    v.reserve(size_t(list.size()));
    for (const QString &s : list) {
        const QByteArray u = s.toUtf8();
        v.push_back(rust::String(u.constData(), size_t(u.size())));
    }
    return v;
}

} // namespace qtrust
