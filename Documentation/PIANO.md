# RooTree — piano di progetto (bozza del 25/10/2026)

File manager completo per SailfishOS, con accesso root e archivi remoti.
Distribuzione: **OpenRepos** (niente vincoli Harbour del Jolla Store).

## Decisioni già prese dall'utente
| Tema | Scelta |
|---|---|
| Distribuzione | OpenRepos, non Jolla Store: niente sandbox, accesso a tutto il filesystem, root possibile |
| Funzioni | navigazione, copia/taglia/incolla, rinomina, elimina, anteprime, ricerca, permessi, archivi, condivisione, file nascosti, segnalibri **+ remoti** (SSH/SFTP, cloud: Dropbox, Mega) |
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
  ownCloud), s3, onedrive, dropbox, pcloud, … **Mega non c'è**: va un crate
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
| 5 | **Remoti** | SFTP/SSH (con chiavi), FTP/FTPS, WebDAV/Nextcloud, SMB, S3; poi i cloud: Dropbox, Mega; copia locale↔remoto con la stessa coda | copia di 1 GB andata e ritorno con hash identico |
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
- **08/11 — RooTree 0.4.0-2 e -3** (chieste dall'utente dopo la Fase 3):
  nella finestra Rinomina/Nuova cartella «Tutto», «Senza estensione»,
  «Copia», «Incolla», e due maniglie per scegliere col dito la parte del
  nome da selezionare (su touch il trascinamento sul TextField non
  seleziona). Provate dall'utente.
- **08/11 — Fase 4 completata: RooTree 0.5.0** (+ sottopacchetto
  `harbour-rootree-root`).
  - L'interfaccia non gira mai da root. «Modalità root» nel menù ⋮ chiede la
    password di sviluppatore e lancia `devel-su rootree-helper --fd 3` in una
    pty (forkpty); la password va solo al terminale di devel-su. L'helper
    (Rust, stesso nucleo) parla solo sul socket ereditato come fd 3, una riga
    JSON per messaggio (`proto.rs`); chiuso il socket annulla i lavori ed
    esce. Senza modalità sviluppatore o senza il pacchetto -root il dialogo
    spiega cosa fare; password sbagliata → messaggio.
  - Accesa: fascia rossa sotto l'intestazione (tocco = spegni); elenco,
    copia/sposta/elimina, rinomina, nuova cartella, archivi, permessi,
    **proprietario e gruppo** (Proprietà) passano dall'helper. I lavori
    dell'helper hanno l'id col bit 1<<30 (dentro un int, per il QML).
    Quello che root crea in una cartella non di root prende il proprietario
    della cartella (la home resta dell'utente).
  - Il visore di testo diventa editor («Modifica»): salvataggio atomico
    (accanto + rename, permessi e proprietario tenuti, un link si salva sul
    file vero), domanda sulle modifiche non salvate; da root l'originale va
    in `/home/debugging/harbour-rootree/copie-di-sicurezza/` (root, 700),
    una copia per file, la più recente.
  - Verifica: 25 test (protocollo e instradamento con l'helper vero su un
    socket, copie di sicurezza, salvataggio atomico, più i 23 di prima). Sul
    POCO con devel-su vero (`rust/examples/prova-root.rs` e
    `tools/collaudo-root`): helper pronto in 51 ms, /root letto,
    **`/etc/rootree-prova.conf` modificato con la copia di sicurezza**
    (proprietario e 640 tenuti), gruppo cambiato, copia da root nella home
    rimasta di defaultuser, password sbagliata riconosciuta, nessun processo
    rimasto dopo lo spegnimento. File di prova tolti.
  - Note: in una cartella leggibile solo da root l'aggiornamento automatico
    (QFileSystemWatcher dell'app) non c'è: si aggiorna dal menù. La ricerca
    gira ancora come utente.
- **08/11 — Fase 5, step 1: interfaccia delle connessioni remote (0.6.0-1).**
  Fase 5 divisa a step su richiesta dell'utente; stile ispirato a Solid
  Explorer (procedura a pagine, riquadri per tipo), resa nello stile di
  RooTundra.
  - Menù delle memorie: sezione «Rete e cloud» con le connessioni salvate
    (simbolo del tipo, nome, utente@server) e il loro menù ⋮ (Modifica,
    Duplica, Elimina con conferma); «Aggiungi una connessione…».
  - Procedura a tutto schermo, quattro passi con indicatore e filo LED che
    avanza: Tipo (riquadri Server: SFTP, FTP/FTPS, WebDAV, Nextcloud/ownCloud,
    SMB, S3; Cloud: Dropbox, Mega) → Server (porta
    predefinita per tipo, FTPS/HTTPS con la porta che segue, condivisione e
    dominio per SMB, endpoint/bucket/regione per S3; saltato dai cloud) →
    Accesso (password, chiave SSH con controllo del file, anonimo/ospite,
    accesso dal browser per i cloud OAuth) → Nome e riepilogo.
  - `Remotes` (C++): profili in `~/.config/…/rootree/connessioni.json` (600).
    Le password non vanno mai nel file: per ora restano in memoria per la
    sessione; la cassaforte cifrata arriva con il collegamento vero.
  - Collaudo senza dito sul POCO (QML di prova col binario vero): componenti
    compilati, salva/descrivi/duplica/modifica/elimina, password assente dal
    file. Toccare una connessione per ora dice che il collegamento arriva
    nel prossimo step.
  - Step successivi (l'utente l'08/11 ha chiesto di invertire 4 e 5): 2
    collegamento SFTP e cassaforte, 3 copia locale↔remoto nella stessa coda,
    4 cloud (Dropbox con accesso dal browser; Mega),
    5 WebDAV/Nextcloud, FTP/FTPS, SMB, S3.
- **08/11 — Fase 5, step 2: collegamento SFTP e cassaforte (0.6.1-1).**
  - SFTP con il crate `ssh2` (libssh2 compilata dal crate col cross-gcc,
    contro l'OpenSSL 3 di sistema; nel target libssh2 non c'è). Scelto al
    posto di OpenDAL, che per SFTP guiderebbe il programma `ssh` esterno.
    L'app ora chiede libssl/libcrypto 3 e libz.
  - Percorsi remoti finti `/@remote/<id>/<percorso sul server>`: barra del
    percorso (parte dal nome della connessione), cronologia e «su» (fermo
    alla radice del server) funzionano come in locale. Per ora sola lettura
    (FolderModel.readOnly): aprire e copiare arriva con lo step 3.
  - Impronta del server come ssh: ~/.ssh/known_hosts (anche `[host]:porta`);
    server nuovo → finestra con tipo e impronta SHA256, «Fidati e collega»
    la aggiunge; impronta cambiata → rifiuto, con la spiegazione (ssh-keygen
    -R). Nessuna credenziale parte prima del controllo.
  - Accesso con password o chiave (anche con frase, chiesta al bisogno);
    collegamento su un thread; una caduta di rete si ricollega da sola una
    volta coi dati della sessione; «Scollega» nel menù della connessione.
  - Cassaforte (`vault.rs`): file cifrato con una password principale scelta
    dall'utente (Argon2id 32 MiB/3 passate → XChaCha20-Poly1305), scritto
    accanto e rinominato, 600; si apre una volta per sessione, «Chiudi la
    cassaforte» nel menù ⋮. Non Sailfish Secrets: è Qt 5 con un D-Bus privato.
  - Verifica: 27 test (cassaforte: niente in chiaro nel file, password
    sbagliata, chiusa = illeggibile). Sul POCO contro il NAS vero (porta non
    standard, chiave ed25519 OpenSSH): collegato in 329 ms,
    elenco cartella iniziale e radice; con un known_hosts vuoto server nuovo
    → fiducia → collegato, poi impronta alterata → rifiuto. Col binario vero
    e un QML di prova: tutto il percorso dei pulsanti, Proprietà di un file
    remoto, Scollega. Trappola: env-rust.sh dava pkg-config del target anche
    ai test (host i686): ora le variabili hanno il suffisso del target; nel
    motore serve `openssl-devel` (installato).
- **08/11 — Fase 5, step 3: copia fra telefono e server (0.6.2-1).**
  - `vfs.rs`: le operazioni di base (metadati, elenco, apri, crea, rinomina,
    cancella, permessi, data, link) guardano il percorso: `/@remote/…` via
    SFTP, il resto come prima. Il motore delle operazioni usa vfs al posto di
    std::fs, quindi copia/sposta/elimina vanno fra telefono e server (e sullo
    stesso server con rename) con la stessa coda, Annulla, conflitti e
    `.rootree-part`. Rinomina fra filesystem diversi → EXDEV → copia.
  - Sul server: rinomina, nuova cartella, permessi; il testo si legge e si
    salva direttamente sul server (atomico: .part + rename, permessi tenuti);
    gli altri file si scaricano nella cache (`…/remoti/<hash>`, pulita dopo
    un giorno) e si aprono; Condividi scarica e condivide. Estrarre,
    comprimere, ricerca e anteprime: non ancora sul server.
  - Con la modalità root accesa, i lavori che toccano un server restano
    nell'app (l'helper non ha le connessioni).
  - **Criterio d'uscita: 1 GB andata e ritorno col NAS vero, SHA-256
    identico** (anche gli altri file, permessi compresi); su 6,8 MB/s in su,
    11,7 MB/s in giù via WiFi. Conflitto «tieni entrambi», rinomina e
    spostamento sul server, spostamento server→telefono, Annulla a metà
    senza resti, eliminazione ricorsiva: provati. Col binario vero e un QML
    di prova: nuova cartella, incolla, rinomina, lettura e modifica del
    testo sul server, permessi, download per aprire, eliminazione.
  - ⛔ Trovato provando: OpenSSH scambia gli argomenti di SSH_FXP_SYMLINK.
    La prima versione ha creato un link fuori posto nella home dell'utente
    sul NAS (tolto subito); ora l'ordine è quello di OpenSSH, provato.
- **08/11 — Fase 5, step 4 (cloud, uno alla volta — scelta dell'utente):
  Dropbox (0.6.3-1).** Il più semplice: API per percorso, PKCE senza
  segreto.
  - Regole verificate l'08/11 sulle pagine ufficiali (fonti in `oauth.rs`):
    Microsoft: `http://localhost`
    ammesso, porta ignorata. Dropbox: PKCE senza segreto, `offline` per il
    rinnovo, indirizzo di ritorno esatto ⇒ porta fissa 53682. Limite di
    utenti delle app Dropbox in sviluppo: non nella pagina ufficiale, da
    verificare.
  - `oauth.rs` (accesso dal browser: ascolto su 127.0.0.1, controllo dello
    state, scambio del codice, rinnovo), `http.rs` (ureq 3 con l'OpenSSL di
    sistema), `cloud.rs` (trait Drive + Dropbox: elenco a pagine, metadati,
    scaricamento in streaming, caricamento a pezzi da 8 MB mentre si scrive,
    nuova cartella, elimina, sposta). `remote.rs` ha ora uno strato generico
    (SFTP o cloud) usato da vfs: copia, coda, Annulla e conflitti valgono
    uguali. Il segreto della connessione è il token di rinnovo, nella
    cassaforte.
  - Ogni utente usa la sua App key gratuita (la procedura spiega come
    crearla: costo zero, nessun servizio a carico dell'autore).
  - Verificato senza account: 30 test (date, intestazione con accenti,
    codifica, ascolto su 127.0.0.1 con browser simulato che rifiuta state
    falsi e riconosce l'accesso negato); HTTPS dal POCO verso Dropbox,
    Microsoft; col binario vero: indirizzo d'accesso, Annulla,
    collegamento senza token. ⏳ La prova con un account vero aspetta la App
    key dell'utente. OneDrive, Mega: «in arrivo».
- **08/11 — Fase 5, step 4: OneDrive (0.6.4-1).** Microsoft Graph per
  percorso (`/me/drive/root:/a/b:`), elenco a pagine (@odata.nextLink),
  scaricamento seguendo il rinvio all'indirizzo già autorizzato,
  caricamento fino a 4 MB con un PUT, sopra con una sessione e pezzi da
  10 MiB (multipli di 320 KiB) sull'uploadUrl senza Bearer; Microsoft vuole
  il totale nel Content-Range, quindi il motore ora passa la dimensione del
  file fino al cloud (`vfs::create(…, size)`). Accesso con ritorno su
  `http://localhost:<porta>` senza barra finale (registrato
  «http://localhost», porta ignorata), ascolto anche su ::1. 32 test; col
  binario vero: indirizzo d'accesso, scope, Annulla. ⏳ Prova con account
  vero: serve l'ID dell'app dell'utente.
- **08/11 — OneDrive TOLTO (0.6.5-1, decisione dell'utente):** seguire il
  portale Microsoft è troppo complicato. Via codice, icone e voce nella
  procedura; il parametro `size` di `vfs::create` resta. **Dropbox provato
  con account vero: funziona.** «Accedi con un account di lavoro» = app con
  permessi di team: la procedura ora elenca le caselle esatte.
- **08/11 — Fase 5, step 4: Mega (0.6.7-1).** Crate `mega` 0.8 (MIT/Apache-2.0, asincrono, un runtime tokio e tubi `sluice` verso il motore sincrono); email e password, password nella cassaforte; albero ricordato 30 s; elimina/sovrascrive = cestino di Mega. Codice a 6 cifre della verifica in due passaggi chiesto a ogni collegamento (0.6.7-2), mai salvato; casella «non chiedere più» (0.6.7-3): sessione nella cassaforte, revoca da Opzioni per Remoto anche sui server di Mega. **Provato con account vero (0.6.7-4): funziona.**
- **08/11 — Google Drive TOLTO (0.6.6-2, decisione dell'utente):** registrare
  l'app sulla console di Google è troppo complicato. Scritto e installato per
  pochi minuti, mai committato. Niente cloud che chiedono di registrare
  un'app su console complicate.
- **08/11 — Fase 5, step 4 chiuso: restano Dropbox e Mega.** Sul POCO
  harbour-rootree 0.6.7-4 + harbour-rootree-root, `rpm -V` pulito (controllato
  il 09/11).
  - Prossimo: step 5 (WebDAV/Nextcloud, FTP/FTPS, SMB, S3), quando lo decide
    l'utente.
  - Da correggere: il menù mostra ancora 0.6.4.
  - RPM armv7hl dell'app fermi alla 0.3.1: si riprendono alla Fase 6.
- **09/11 — Fase 5, step 5: WebDAV e Nextcloud/ownCloud (0.6.8-5).** `webdav.rs`
  implementa `Drive` (PROPFIND, GET, PUT in streaming, MKCOL, MOVE, DELETE), quindi
  copia, coda, Annulla e conflitti valgono come per gli altri remoti. Icona e nome
  propri per Nextcloud. Cestino di Nextcloud (⋮ → Opzioni per Remoto): elenco,
  ripristino, eliminazione definitiva, svuotamento. Su WebDAV e cloud una cartella
  si elimina con una richiesta sola; il motore legge i dati con l'elenco della
  cartella invece che voce per voce. Verifica: 33 test; col motore vero dal POCO
  contro Nextcloud 34 (utente temporaneo): 30 MB andata e ritorno identici, nomi
  con accenti e simboli, Annulla senza resti, 30 file in 3 cartelle, cestino.
  Provato dall'utente col dito, anche in HTTPS (0.6.8-6 aggiunge la procedura per la
  password per app). Restano FTP/FTPS, SMB, S3.
- **10/11 — Fase 5, step 5: FTP e FTPS (0.6.9-1).** `ftp.rs` implementa `Drive`
  col crate `suppaftp` (MIT/Apache-2.0) e un connettore TLS nostro su OpenSSL:
  FTPS esplicito (AUTH TLS) e implicito (porta 990), accesso anonimo. Il canale
  dati riusa la sessione TLS del SUO controllo (vsftpd e ProFTPD la pretendono di
  serie), TLS 1.2 al massimo (in TLS 1.3 OpenSSL usa ogni biglietto una volta
  sola e il secondo canale dati veniva rifiutato). Certificati fatti in casa:
  impronta SHA-256 da confermare la prima volta (`certificati-ftps.json` accanto
  alla cassaforte), un cambio viene rifiutato senza mandare la password. Un
  collegamento per sfogliare e uno per ogni trasferimento (si sfoglia durante una
  copia), con fino a due collegamenti finiti tenuti pronti: 30 file piccoli in
  0,7–0,95 s invece di 13–16. Annulla toglie dal server il file a metà; i
  collegamenti chiusi dal server per inattività si riaprono da soli.
  Verifica: 35 test; dal POCO col motore vero contro ProFTPD 1.3.8 in un
  container sul PC, in FTPS e in chiaro: 30 MB andata e ritorno identici, nomi con
  accenti e simboli, file nascosti, Annulla, 30 file in 3 cartelle andata, ritorno
  ed eliminazione, anonimo in sola lettura, ricollegamento dopo TimeoutIdle,
  certificato cambiato rifiutato. ⚠️ FTPS implicito NON provato (la porta 990
  non si apre da un container senza root). vsftpd nel container crollava da solo
  anche con curl (segfault del processo principale): non usato per le prove.
  ✅ Collaudata coi tocchi sintetici dal POCO (0.6.9-2: l'editor perdeva l'ultima
  parola, che la tastiera teneva ancora in composizione).
- **10/11 — Fase 5, step 5: SMB 2 e 3 (0.7.0-1).** `smb.rs` implementa `Drive` col
  crate `smb2` (MIT/Apache-2.0, Rust puro: nessuna libreria nuova nel pacchetto):
  firma e cifratura SMB3, NTLM, utente o ospite, dominio facoltativo. Un collegamento
  solo: SMB fa viaggiare più richieste insieme, e letture e scritture hanno la loro
  copia della connessione, quindi si sfoglia durante una copia. Percorsi: `/` è
  l'elenco delle condivisioni (la condivisione nella connessione ora è facoltativa),
  `/Condivisione/…` è dentro. Lettura a flusso con richieste in parallelo; scrittura
  a pezzi da 1 MB; Annulla toglie il file a metà; fra due condivisioni si copia.
  Una password sbagliata su un Samba «map to guest» non diventa un ospite muto: è
  «password rifiutata». Se il server si riavvia, il collegamento si riprende da solo.
  Verifica: 37 test; dal POCO col motore vero contro Samba 4.17 in un container sul
  PC (porta 4450): password sbagliata, condivisione assente, elenco delle
  condivisioni, 30 MB andata e ritorno identici (~11 MB/s in Wi-Fi), elenco durante
  una lettura, Annulla senza resti, nomi con accenti e simboli, file nascosti, 30
  file in 3 cartelle andata, ritorno ed eliminazione, condivisione con cifratura
  obbligatoria, ospite in sola lettura, Samba riavviato a metà (ripresa in 33 ms).
  Poi coi tocchi sintetici: procedura guidata, elenco delle condivisioni, copia da
  una condivisione a quella cifrata (md5 identici), eliminazione, modifica di un file.
  Insieme: la tastiera nella procedura non copre più i pulsanti e si chiude con un
  tocco fuori o con Invio (con Qt 6 il plugin Maliit dà un'area 0x0: si usa
  l'altezza misurata). Resta S3: con quello, la 0.8 (decisione dell'utente).
- **12/11 — Cestino in ogni memoria, editor di testo, Nuovo file (0.7.1-1 → -6).**
  Fuori dal piano, su richiesta dell'utente. `trash.rs`: cestino freedesktop per la
  home, per ogni altro montaggio (`.Trash-<uid>`: memoria Android, SD, USB), per
  root (`.Trash-0`, di root e 700) e per i server senza cestino proprio
  (`.Trash-rootree` nella cartella iniziale, in cima a ogni condivisione SMB);
  Nextcloud, Mega e Dropbox usano il loro. Elimina = nel cestino, con la casella
  «Elimina definitivamente»; svuotamento da solo dopo 30 giorni (7/30/90/mai).
  Menù ⋮: «Nuovo file», «Cestino» nella sezione Cartella, «Opzioni per Remoto» in
  una sezione sua. Il visore/editor di testo ora si comporta da editor: tocco =
  cursore, tocco fuori = via la selezione, doppio tocco o pressione lunga = parola,
  maniglie e pressione lunga + trascina per allargare anche su più righe, barra
  Tutto/Copia/Taglia/Incolla agganciata in fondo.
  Verifica: 40 test Rust (3 nuovi sul cestino); sul POCO senza schermo
  (`tools/collaudo-cestino`) interna, Android, SD e root; l'editor sul PC con tocchi
  veri (`tools/collaudo-editor`, PySide6, 15 controlli). Non provati sul vivo i
  cestini dei server e dei cloud.
- **14/11 — Fase 5 chiusa: S3 e compatibili (0.8.0-1).** `s3.rs` implementa `Drive`
  con la firma AWS SigV4 scritta a mano sopra ureq (nessuna libreria S3; solo `hmac`
  e `md-5`, già nel lockfile). Endpoint vuoto = Amazon (indirizzo per regione, stile
  «bucket.s3.regione», a percorso se il bucket ha punti); con endpoint (anche
  `http://indirizzo:porta`) a percorso. Senza bucket la radice è l'elenco dei bucket:
  «Nuova cartella» lì crea un bucket, eliminarlo toglie contenuto e bucket; il
  cestino di RooTree va in cima a ogni bucket (come le condivisioni SMB). Cartelle =
  prefissi, cartella vuota = oggetto «nome/»; rinomina = copia sul server + elimina
  (oltre 5 GiB il motore scarica e ricarica); fino a 5 GiB una PUT a flusso, oltre a
  pezzi (≥16 MiB, ≤10 000); Annulla annulla anche il caricamento a pezzi. Regione:
  quella del profilo, o imparata da Amazon (intestazione o corpo dell'errore); vuota
  su un servizio che la rifiuta senza dirla ⇒ si provano «garage» e «auto».
  Verifica: 3 test nuovi (la firma dà esattamente quella dei due esempi della
  documentazione AWS), 43 verdi; dal POCO col motore vero contro Garage 2.1 in un
  container sul PC (MinIO non distribuisce più immagini): chiavi sbagliate (AUTH),
  bucket assente, elenco/creazione/eliminazione di bucket, 30 MB andata e ritorno
  identici (~11 MB/s in Wi-Fi), a pezzi 40 MB identici, Annulla senza resti (a flusso
  e a pezzi), rinomina di file e cartelle, nomi con accenti e simboli, file vuoto,
  cartella doppia = «esiste già», copia/ritorno/spostamento/eliminazione di 30 file
  con una cartella vuota, cestino (dentro, elenco, ripristino). Non provati: Amazon
  vero, B2, Wasabi, R2, file oltre 5 GiB.
