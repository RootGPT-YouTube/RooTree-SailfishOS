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
- **25/10 — Fase 2 completata: RooTree 0.3.0.**
  - Anteprime (C++, `image://thumb`, 2 thread, cache freedesktop
    `~/.cache/thumbnails` con Thumb::MTime, fallimenti in `fail/rootree`):
    immagini con Qt (rotazione EXIF) e con il nucleo Rust per WebP/TIFF/BMP;
    video con GStreamer di sistema (fotogramma al 10%, decodificatori droid
    esclusi); PDF con poppler di sistema. Nella griglia: cornice d'acciaio e
    icona del tipo nell'angolo; si spengono dal menù.
  - Visore immagini (`image://picture`: anche WebP/TIFF), con scorrimento fra
    le foto della cartella, zoom con due dita e doppio tocco. Visore di testo
    (2 MB, a capo attivabile, i binari si riconoscono).
  - Apri con: app predefinita di Sailfish (lca-tool), elenco con icone (prima
    il tema glacier, poi sailfish-default) e nomi tradotti dai cataloghi Jolla,
    «Usa sempre per…». Condividi con il dialogo di sistema.
  - Sandbox: le app di Sailfish vedono solo Documents/Downloads/Music/
    Pictures/Public/Videos e le schede; per i file altrove un collegamento
    fisso in `~/Downloads/.rootree` (copia fino a 200 MB se non si può),
    ripulito dopo un giorno.
  - Collaudo sul POCO: anteprime di MP4 H.264, WebM VP9, PDF, JPEG, PNG,
    WebP, TIFF; visore con scorrimento e zoom; testo; Condividi apre il
    dialogo di sistema; Apri con → Note; video → mpv e PDF → Documenti
    attraverso il collegamento fisso. armv7hl: nel chroot a 32 bit l'app
    parte e fa le anteprime di PDF e WebP.
  - Da chiarire: in offscreen (solo lì) Qt avvisa di larghezze NaN.
- **27/10 — RooTree 0.3.1: ritocchi chiesti dall'utente prima della Fase 3.**
  - Menù delle memorie: ogni memoria (interna, Android, SD, USB) si apre con
    la sua freccia sulle cartelle predefinite che ha davvero (Documents,
    Downloads, Pictures, Music, Videos; anche Download e Movies). Il menù si
    apre sempre tutto chiuso. La freccia accanto al disco della memoria non
    finisce più sotto il disco.
  - Temi, dal menù ⋮: RooTundra (predefinito) o «SailfishOS classico». Il
    secondo è piatto, con i colori dell'ambience letti da dconf, Sail Sans Pro
    del sistema e le icone dell'app ricolorate. Niente icone di Jolla e
    niente immagine di sfondo, per scelta dell'utente. Il cambio è immediato.
  - Collaudo sul POCO: griglia, menù ⋮, menù delle memorie con le cartelle
    (anche della SD) e cambio di tema dal vivo nei due versi. Ancora da
    guardare col tema Sailfish: Proprietà, dialoghi, visori.
- **08/11 — Fase 3 completata: RooTree 0.4.0.**
  - Ricerca (nucleo Rust, `search.rs`, un thread suo): per nome (testo o
    `*`/`?`), espressione regolare sul nome, testo o espressione regolare nel
    contenuto (file fino a 64 MB, i binari si saltano, la riga trovata nel
    risultato). Dalla cartella corrente in giù, senza seguire i link alle
    cartelle e senza entrare in /proc, /sys, /dev; si ferma a 5000 risultati.
    Interfaccia: pannello a tutto schermo dal pulsante con la lente; tocco =
    apri, disco a destra = vai alla cartella che lo contiene.
  - Archivi (`archive.rs`): zip, tar (.gz .bz2 .xz .zst) e 7z in Rust
    (lettura e creazione; zstd in C col cross-gcc); un file compresso da solo
    (`.gz` ecc.) come archivio di una voce; RAR, iso, cpio, cab, lha, deb, rpm
    in sola lettura con la libarchive di sistema caricata con dlopen (nel
    target non ci sono gli header; lo spec la chiede). Toccare un archivio lo
    apre come una cartella; copiare da dentro = estrarre quelle voci;
    «Estrai qui» e «Comprimi…» (7 formati) nel menù Altro. Si estrae in una
    cartella temporanea nascosta, i nomi con `..` si saltano, i link simbolici
    si creano per ultimi, niente setuid; una voce sola in cima si estrae
    senza cartella in più, altrimenti in una cartella col nome dell'archivio.
  - Ordinamento (richiesta dell'utente dell'08/11): per data l'ordine normale
    è dal più recente (predefinito); interruttore «Solo per questa cartella»
    (acceso di base: l'ordine scelto vale per la cartella; spento: l'ordine
    vale ovunque e gli ordini delle singole cartelle si azzerano).
  - Verifica: 23 test (`cargo test --release -- --include-ignored` nel
    motore), fra cui il **confronto con bsdtar 3.8.6** nei due versi: archivi
    di bsdtar (tar, tar.gz/bz2/xz/zst, zip, 7z, cpio, iso) e il RAR della suite
    di libarchive estratti da RooTree = estratti da bsdtar; archivi di RooTree
    (7 formati) estratti da bsdtar = sorgente (7z senza i link). Archivio
    costruito apposta con `../` e un link che esce: niente scritto fuori.
  - POCO: RPM 0.4.0-1 installato, `rpm -V` pulito; il QML si carica (prova
    fuori schermo, che ha trovato un errore poi corretto). Prove col dito
    dell'utente: archivi di esempio in
    `/home/debugging/2026-11-08-rootree-fase3/archivi-di-prova`.
