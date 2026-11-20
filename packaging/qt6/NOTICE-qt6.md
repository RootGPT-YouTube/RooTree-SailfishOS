## Qt 6.12 incluso in RooTree (`/home/rootree/qt6`)

Il pacchetto contiene anche librerie compilate, senza modifiche al loro codice
salvo dove indicato, per Sailfish OS 5.1.

| Componente | Licenza | Sorgenti |
|---|---|---|
| Qt 6.12.0 (qtbase, qtdeclarative, qtsvg, qtshadertools, qtvirtualkeyboard) | LGPL-3.0 (alcune parti GPL-3.0) | tarball ufficiali https://download.qt.io/official_releases/qt/6.12/6.12.0/submodules/ ; patch e script di build in https://github.com/RootGPT-YouTube/RooTitanium-SailfishOS (`packaging/qt612/`) |
| Plugin Maliit per Qt (platform input context) | LGPL-2.1 | chum `qt-sfos-maliit-platforminputcontext` commit 9bb86e6 + patch `maliit-qt6.10-guiprivate.patch` (in RooTitanium-SailfishOS, `packaging/harbour-rootitanium/`) + patch [`maliit-container-nullptr.patch`](maliit-container-nullptr.patch) (dalla 0.9.7-6) |
| ICU 73 | Unicode-3.0 | https://github.com/unicode-org/icu |
| HarfBuzz, FreeType, Fontconfig, Graphite2 | MIT / FTL / MIT / LGPL-2.1 | pacchetti di Sailfish OS 5.1 |
| GLib, D-Bus, PCRE2, libxkbcommon, zlib, zstd, bzip2, libpng, libjpeg-turbo, expat | LGPL-2.1 / AFL-2.1 o GPL-2.0 / BSD / MIT / zlib / BSD / bzip2 / libpng / IJG-BSD / MIT | pacchetti di Sailfish OS 5.1 |

Per l'LGPL: le librerie sono caricate dinamicamente dalle app e si possono
sostituire con versioni proprie compatibili mettendole in `/home/rootree/qt6/lib`.
