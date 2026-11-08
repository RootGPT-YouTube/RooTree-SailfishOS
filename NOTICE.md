# RooTree — componenti di terze parti

RooTree è software libero sotto **GPL-3.0-or-later** (vedi `LICENSE`).
Contiene o usa anche:

| Componente | Licenza | Dove |
|---|---|---|
| Font Awesome Free 5.15.4 (icone SVG), © Fonticons, Inc. | CC BY 4.0 — https://fontawesome.com/license/free | `app/qml/icons/`, `grafica/glifi/` (ogni file porta l'attribuzione); disegni incisi nelle icone dei tipi in `app/qml/icons/tipi/`. I loghi Android, USB, Google Drive, Dropbox, Microsoft (per OneDrive), AWS (per S3) e Windows (per SMB) sono marchi dei rispettivi titolari e indicano solo il servizio a cui si riferiscono. |
| Rajdhani, © 2014 Indian Type Foundry | SIL Open Font License 1.1 | `app/qml/fonts/` (testo della licenza in `OFL-Rajdhani.txt`) |
| `SteelDisc.qml`, `FrostCorner.qml` e lo stile grafico, da RooTundra © 2026 RootGPT | BSD (vedi il repository RooTundra) | `app/qml/` |
| cxx (ponte Rust/C++), libc (crate) | MIT o Apache-2.0 | dipendenze di `rust/` |
| image (crate Rust, WebP/TIFF/BMP) | MIT o Apache-2.0 | dipendenza di `rust/` |
| GStreamer 1.x (anteprime dei video) | LGPL-2.1-or-later | non incluso: librerie di sistema di Sailfish OS, collegate dinamicamente |
| poppler (anteprime dei PDF) | GPL-2.0-or-later | non incluso: libreria di sistema di Sailfish OS, collegata dinamicamente |
| Qt 6.12 | LGPL-3.0 | non incluso: lo fornisce il pacchetto `rootgpt-qt6-runtime` (vedi il suo NOTICE) |
