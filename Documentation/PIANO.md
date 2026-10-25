# RooTree — piano di progetto (bozza del 25/10/2026)

File manager completo per SailfishOS, con accesso root e archivi remoti.
Distribuzione: **OpenRepos** (niente vincoli Harbour del Jolla Store).

## Decisioni già prese dall'utente
| Tema | Scelta |
|---|---|
| Distribuzione | OpenRepos, non Jolla Store: niente sandbox, accesso a tutto il filesystem, root possibile |
| Funzioni | navigazione, copia/taglia/incolla, rinomina, elimina, anteprime, ricerca, permessi, archivi, condivisione, file nascosti, segnalibri **+ remoti** (SSH/SFTP, cloud: Google Drive, Mega, OneDrive…) |
| Stack | Qt6 / QML / C++ / Rust, **Silica non obbligatoria** (come RooTitanium) |
| Architetture | aarch64 + armv7hl |
| Icona | cartella hi-tech «acciaio / tech» come RooTheater e RooTelegram → `grafica/icona-vettoriale.py`, **approvata** |
| Root | un'**opzione «root»** nell'app che attiva i privilegi su richiesta (25/10) |
| Navigazione | **menù a tendina** delle memorie: Memoria interna, Memoria Android, Altre memorie (scheda SD, Cloud, SSH, SFTP, Nextcloud, …) (25/10) |
| Costi | **costo zero per l'autore**: app gratuita, nessun servizio a suo carico; le chiavi cloud che servono le inserisce l'utente (25/10) |
| Licenza / repo | GPL-3.0; GitHub `RootGPT-YouTube/RooTree-SailfishOS` **privato fino al rilascio** (25/10) |
| Archivi | comprimere e decomprimere **file e cartelle** (25/10) |
| Navigazione | **a cartelle, come Dolphin**, non ad albero: percorso a briciole, indietro/avanti/su, vista a icone o a dettagli (25/10) |
| Stile | quello di **RooTundra**: pannelli e pulsanti d'acciaio, LED blu elettrico, ghiaccio e brina, font Rajdhani. Icone dei file: **le sagome classiche di Dolphin, ma d'acciaio** (non dischi rotondi) (25/10) |
| Qt | stack Qt 6.12 di RooTitanium, in un **pacchetto runtime condiviso** fra le app Qt6 di RootGPT (25/10) |

## Architettura proposta

```
┌──────────────────────────── QML (UI) ─────────────────────────────┐
│ pagine, liste, dialoghi, anteprime, impostazioni — Qt Quick puro  │
└──────────────────────────────┬────────────────────────────────────┘
                               │ cxx (ponte C++ ↔ Rust)
┌──────────────────────────────▼────────────────────────────────────┐
│ C++ sottile: main, launcher, image provider anteprime, D-Bus SFOS │
│ (condivisione, «Apri con», notifiche)                             │
├───────────────────────────────────────────────────────────────────┤
│ nucleo Rust (rootree-core)                                         │
│  • VFS unico: locale · root · SFTP · FTP · WebDAV · S3 · cloud    │
│  • coda operazioni (copia/sposta/elimina) con avanzamento,        │
│    annullamento, conflitti, ripresa                               │
│  • ricerca (nome, regex, contenuto) · archivi · hash · permessi   │
│  • credenziali cifrate, OAuth con redirect su 127.0.0.1           │
└──────────────┬────────────────────────────────────────────────────┘
               │ socket Unix, protocollo minimo
┌──────────────▼──────────────┐
│ rootree-helper (Rust, root) │  ← solo nel sottopacchetto «-root»
└─────────────────────────────┘
```

- **Rust ↔ Qt: crate `cxx`** (scelto il 25/10 al posto di cxx-qt, che dovrebbe
  eseguire `moc` del target dentro cargo). Il nucleo Rust è una libreria statica;
  modelli e oggetti QML sono in C++ e chiamano il nucleo attraverso il ponte.
- **Remoti: OpenDAL** (Apache-2.0), un'unica API per sftp, ftp, webdav (Nextcloud,
  ownCloud), s3, gdrive, onedrive, dropbox, pcloud, … **Mega non c'è**: va un crate
  a parte (`mega`, da valutare) o rclone come ripiego.
- **Archivi** (comprimere e decomprimere file e cartelle): crate `zip`, `tar`, `flate2`, `xz2`, `zstd`, `bzip2`, `sevenz-rust2`;
  la RAR solo in estrazione, tramite libarchive (BSD).
- **Root**: l'interfaccia grafica non gira **mai** come root. Quando l'utente
  accende l'opzione «root», l'app avvia un helper privilegiato separato, che fa
  solo le operazioni sul filesystem e risponde all'app su un socket. Si spegne con
  l'opzione o alla chiusura dell'app. Finché è acceso, una fascia rossa lo segnala.
  L'accensione chiede la password di `devel-su` (quella della modalità
  sviluppatore, deciso il 25/10). Se la modalità sviluppatore è spenta, l'app lo
  dice e spiega come attivarla (Impostazioni → Strumenti per sviluppatori,
  impostare la password).
- **Packaging**: lo schema già collaudato con RooTitanium. Launcher ELF in
  `/usr/bin/harbour-rootree`, `.desktop` con `[X-Sailjail] Sandboxing=Disabled`
  (indispensabile: con sailjail l'app vedrebbe solo le sue cartelle), Qt 6.12
  incluso nel bundle.

## Fasi

| # | Fase | Contenuto | Esito verificabile |
|---|---|---|---|
| 0 | **Fondamenta** | repo git + `.gitignore`, toolchain Rust cross per aarch64/armv7hl, «hello world» Rust (cxx) + QML sul POCO, launcher + `.desktop`, bundle Qt, spec RPM, icona | RPM che si installa e si avvia dall'icona sul POCO, sulle 2 arch |
| 1 | **Browser locale** | navigazione, elenco/griglia, ordinamento, file nascosti, segnalibri, punti di montaggio (SD, Android), dettagli e permessi, nuova cartella, rinomina, elimina, selezione multipla, copia/taglia/incolla con coda e conflitti | gestire a mano un albero di prova senza perdere un byte (hash prima/dopo) |
| 2 | **Aprire e vedere** | anteprime (immagini, video, PDF), visore di testo/immagini, «Apri con», condivisione di Sailfish | aprire e condividere dal browser i tipi di file comuni |
| 3 | **Ricerca e archivi** | ricerca per nome/regex/contenuto, crea ed estrai zip/tar.*/7z, estrai rar, sfoglia l'archivio come una cartella | estrazione a confronto con quella di `bsdtar` |
| 4 | **Modalità root** | helper, sottopacchetto `-root`, permessi e proprietario, avviso visivo chiaro quando si è in root | modificare un file di `/etc` con un backup automatico |
| 5 | **Remoti** | SFTP/SSH (con chiavi), FTP/FTPS, WebDAV/Nextcloud, SMB, S3; poi i cloud: Google Drive, OneDrive, Dropbox, Mega; copia locale↔remoto con la stessa coda | copia di 1 GB andata e ritorno con hash identico |
| 6 | **Rifinitura e rilascio** | italiano/inglese, impostazioni, cover, eventuale doppio pannello, pagina OpenRepos | primo rilascio pubblico |

## Punti delicati già noti (dalle sessioni di RooTitanium)
- **sailjail** rifiuta gli script e vuole un ELF in `/usr/bin`, quindi serve il
  launcher compilato.
- **sb2 + qmlcachegen/rcc**: creano file con permessi 000, quindi serve il
  sorvegliante già presente in `RooTitanium/packaging/qt612/build-module.sh`.
- **Tastiera**: il plugin Maliit va ricompilato per Qt 6.12. Esiste già, insieme
  alla patch.
- **Rust nel target**: il repo Jolla di 5.1.0.11 ha solo rust 1.75, troppo
  vecchio per OpenDAL. Risolto: rustup i686 nel build engine e il linker cross
  del SDK.
- **armv7hl**: Qt 6 di piggz è compilato su SFOS 5.0 (ICU 70) e c'è la
  compatibilità in `RooTitanium/packaging/icu70-compat/`.
- **Google Drive (da verificare)**: lo scope di accesso completo a Drive è
  «restricted». Un'app pubblica con un client OAuth dell'autore richiede la
  verifica di Google, con valutazione di sicurezza a pagamento. Senza verifica
  si resta a 100 utenti e compare la schermata «app non verificata». Strada a
  costo zero: ogni utente inserisce il **proprio** client ID, come fa rclone.
  OneDrive e Dropbox: la registrazione dell'app è gratuita.

Dopo ogni fase ci si ferma e si avvisa l'utente (25/10).

## Stato
- **25/10 — Fase 0 completata.**
  - Repo git locale, catena di build: `tools/build.sh <arch>` compila il nucleo Rust
    e l'app Qt 6.12; `tools/pacchetto.sh <arch>` fa i due RPM in `RPMS/`.
  - **`rootgpt-qt6-runtime` 6.12.0-1**: Qt condiviso in `/home/rootgpt-qt6`
    (118 MB installato, RPM da 33 MB). Lo assembla
    `packaging/rootgpt-qt6-runtime/assembla.py` partendo dal target `-qt612`.
    Contiene anche il launcher generico (`rootgpt-qt6-launch.c`) che ogni app
    compila per sé.
  - **`harbour-rootree` 0.1.0-1** (312 KB): launcher ELF in `/usr/bin`, app in
    `/usr/share/harbour-rootree`, `.desktop` con `Sandboxing=Disabled`, icona in
    5 misure.
  - POCO (aarch64): installati con `rpm -Uvh`, `rpm -V` pulito. Dal launcher
    l'app parte e mostra la home letta dal nucleo Rust, con la cmdline forgiata
    per la cover.
  - Dall'icona (tocchi sintetici, 25/10): l'app parte, si naviga (dentro una
    cartella e di nuovo su), lo swipe dal bordo la manda in cover, e ritoccando
    l'icona torna avanti la stessa istanza.
  - armv7hl: RPM fatti; provati nel chroot a 32 bit del POCO in modalità
    offscreen (l'app parte, il QML si carica). Su un telefono vero a 32 bit
    non è ancora provata.
- **25/10 — Fase 1 completata: RooTree 0.2.0.**
  - Nucleo Rust: elenco con ordinamento naturale, memorie (interna, Android,
    SD/USB da /proc/mounts, radice), nuova cartella, rinomina che non
    sovrascrive, permessi. Motore delle operazioni in coda (copia, sposta,
    elimina, dimensione) con avanzamento, Annulla, domande su conflitti ed
    errori, copia via file `.rootree-part` e sync ogni 8 MB. 14 test
    (`cargo test` nel motore).
  - Interfaccia: menù a tendina delle memorie (lastra di ghiaccio) con
    segnalibri, briciole, indietro/avanti/su, icone/dettagli, ordina per
    nome/dimensione/data/tipo, file nascosti, selezione con pressione lunga,
    Copia/Taglia/Incolla qui, Elimina con conferma, Rinomina, Proprietà con
    i permessi modificabili, pannello delle operazioni, avvisi.
  - Collaudo sul POCO con i tocchi sintetici: copia di una cartella con hash
    identici; spostamento sulla SD exFAT (filesystem diverso) con hash
    identici e sorgente cancellata; conflitto «Tieni entrambi»; eliminazione
    dalla SD; annullamento di una copia da 400 MB senza resti; Nuova
    cartella e Rinomina con la tastiera di sistema. `rpm -V` pulito.
  - armv7hl: RPM fatti, il QML si carica nel chroot a 32 bit (offscreen).
