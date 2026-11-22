# harbour-rootree — file manager per Sailfish OS (OpenRepos, non Jolla Store).
# Si impacchetta uno staging già pronto (tools/pacchetto.sh), qui non si compila.
# Dalla 0.9.7 un pacchetto solo, completo in sé come RooTitanium: Qt 6.12 è
# dentro, in /home/rootree/qt6, e dalla 0.9.7-2 anche il programma (/home/rootree/app) (la partizione /home: il rootfs dei telefoni è
# piccolo). Prima lo dava il pacchetto a parte rootgpt-qt6-runtime, che ora
# sparisce da solo all'aggiornamento (Obsoletes: lo usava solo RooTree).
# Launcher /usr/bin/harbour-rootree: sailjail rifiuta gli script e vuole un ELF
# in /usr/bin; il .desktop ha [X-Sailjail] Sandboxing=Disabled, altrimenti un
# file manager vedrebbe solo le sue cartelle.

%global _apphome /home/rootree
%global debug_package %{nil}
%define __os_install_post %{nil}
%define __brp_strip %{nil}
%define __brp_strip_static_archive %{nil}
%define __brp_strip_comment_note %{nil}

Name:       harbour-rootree
Version:    0.9.8
Release:    3
Summary:    RooTree — file manager for Sailfish OS
License:    GPLv3+ and LGPLv3 and BSD and MIT
Group:      Applications/System
URL:        https://github.com/RootGPT-YouTube/RooTree-SailfishOS
ExclusiveArch: aarch64 armv7hl
AutoReqProv: no
Obsoletes:  rootgpt-qt6-runtime < 6.13
# 0.9.6: l'helper della modalità root è dentro l'app (prima era il sottopacchetto
# -root, da aggiornare sempre insieme): chi aveva il -root lo vede tolto da solo.
# Niente Requires di devel-su: senza modalità sviluppatore l'app lo dice quando
# si prova ad accendere la modalità root, e RooTree si installa lo stesso.
Obsoletes:  %{name}-root < 0.9.6-1
Provides:   %{name}-root = %{version}-%{release}
# anteprime (GStreamer per i video, poppler per i PDF) e «Apri con»
# (libcontentaction): pacchetti standard di Sailfish OS. AutoReqProv è spento,
# quindi le librerie si chiedono a mano, per soname.
Requires:   gstreamer1.0-plugins-base
Requires:   /usr/bin/lca-tool
# RAR e formati rari (iso, cpio, cab...): libarchive, caricata a runtime con
# dlopen (zip, tar.* e 7z invece sono nel binario)
Requires:   libarchive
# il Qt incluso: le librerie di sistema che non porta (SISTEMA in
# packaging/qt6/assembla.py), come le chiedeva rootgpt-qt6-runtime
%ifarch aarch64
Requires:   libEGL.so.1()(64bit)
Requires:   libGLESv2.so.2()(64bit)
Requires:   libwayland-client.so.0()(64bit)
Requires:   libwayland-cursor.so.0()(64bit)
Requires:   libwayland-egl.so.1()(64bit)
Requires:   libsqlite3.so.0()(64bit)
Requires:   libsystemd.so.0()(64bit)
Requires:   libstdc++.so.6()(64bit)
Requires:   libgcc_s.so.1()(64bit)
Requires:   libgstreamer-1.0.so.0()(64bit)
Requires:   libgstbase-1.0.so.0()(64bit)
Requires:   libgstapp-1.0.so.0()(64bit)
Requires:   libgstvideo-1.0.so.0()(64bit)
Requires:   libgobject-2.0.so.0()(64bit)
Requires:   libpoppler-cpp.so.2()(64bit)
Requires:   libssl.so.3()(64bit)
Requires:   libcrypto.so.3()(64bit)
Requires:   libz.so.1()(64bit)
%else
Requires:   libEGL.so.1
Requires:   libGLESv2.so.2
Requires:   libwayland-client.so.0
Requires:   libwayland-cursor.so.0
Requires:   libwayland-egl.so.1
Requires:   libsqlite3.so.0
Requires:   libsystemd.so.0
Requires:   libstdc++.so.6
Requires:   libgcc_s.so.1
Requires:   libgstreamer-1.0.so.0
Requires:   libgstbase-1.0.so.0
Requires:   libgstapp-1.0.so.0
Requires:   libgstvideo-1.0.so.0
Requires:   libgobject-2.0.so.0
Requires:   libpoppler-cpp.so.2
Requires:   libssl.so.3
Requires:   libcrypto.so.3
Requires:   libz.so.1
%endif

%description
RooTree is a file manager for Sailfish OS: internal memory, Android storage,
SD card, cloud and remote storage, archives, and an optional root mode.
Root mode is unlocked with the developer-mode password: a separate helper,
started with devel-su, performs the file operations as root; the app itself
never runs as root.

%prep

%install
rm -rf %{buildroot}
# 0.9.7-2: anche il programma e il QML stanno in /home/rootree/app, come
# RooTitanium: sul rootfs restano solo lanciatore, helper root, .desktop e icone
mkdir -p %{buildroot}%{_apphome} %{buildroot}%{_bindir}
cp -a %{stagingdir}/app %{buildroot}%{_apphome}/app
cp -a %{stagingdir}/qt6 %{buildroot}%{_apphome}/qt6
install -m0755 %{stagingdir}/launcher %{buildroot}%{_bindir}/%{name}
install -D -m0644 %{stagingdir}/%{name}.desktop %{buildroot}%{_datadir}/applications/%{name}.desktop
for s in 86 108 128 172 256; do
  install -D -m0644 %{stagingdir}/icons/${s}x${s}/%{name}.png \
    %{buildroot}%{_datadir}/icons/hicolor/${s}x${s}/apps/%{name}.png
done
install -D -m0644 %{stagingdir}/LICENSE %{buildroot}%{_defaultlicensedir}/%{name}/LICENSE
install -D -m0644 %{stagingdir}/NOTICE.md %{buildroot}%{_defaultlicensedir}/%{name}/NOTICE.md
# non setuid: lo avvia devel-su, dopo la password di sviluppatore
install -D -m0755 %{stagingdir}/rootree-helper %{buildroot}%{_libexecdir}/%{name}/rootree-helper

%files
%defattr(-,root,root,-)
%license %{_defaultlicensedir}/%{name}/LICENSE
%license %{_defaultlicensedir}/%{name}/NOTICE.md
%{_bindir}/%{name}
%{_apphome}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/*/apps/%{name}.png
%dir %{_libexecdir}/%{name}
%{_libexecdir}/%{name}/rootree-helper

%changelog
* Sun Nov 22 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.8-3
- Mentre scorri da una scheda all'altra, fra le due cartelle compare un
  divisore d'acciaio col filo blu: si vede che si passa all'altra scheda.

* Sun Nov 22 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.8-2
- La scheda SD ora sta nel menù delle memorie accanto a Memoria interna e
  Memoria Android (prima era nascosta dentro «Altre memorie»). Compare solo se
  è inserita; le chiavette USB restano in «Altre memorie».

* Sun Nov 22 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.8-1
- Due schede, A e B: due linguette in cima, come quelle di due cartellette.
  In ognuna si apre una memoria o una cartella diversa (per esempio Memoria
  interna in A e Scheda SD in B), con la sua cronologia Indietro/Avanti.
  Si passa dall'una all'altra toccando la linguetta o con uno swipe a destra o
  a sinistra sulla cartella. La scheda B si riapre dove l'avevi lasciata.
- Copia o taglia in una scheda e incolla nell'altra: gli appunti sono in comune.

* Fri Nov 20 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.7-6
- Corretto il crash al tocco dei campi di testo (password, email…) sui telefoni a
  32 bit come lo Xperia XA2: il difetto era nel collegamento con la tastiera di
  sistema (plugin Maliit per Qt 6), che ora è corretto.

* Wed Nov 18 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.7-5
- Cestino in ogni memoria del vassoio: sotto Memoria interna, Memoria Android,
  scheda SD e chiavette, insieme a Documents, Downloads e le altre cartelle.
- Tastiera: i pulsanti delle procedure non restano più a mezz'aria sui
  telefoni dove il sistema fa già spazio alla tastiera (Xperia e simili).
- Campo email di Mega senza testo predittivo né maiuscola automatica, come
  gli altri campi (possibile cura del crash segnalato sullo Xperia XA2).

* Wed Nov 18 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.7-4
- Espelli dalla cartella della chiavetta: ora torna davvero alla Memoria
  interna ed espelle (nella 0.9.7-3 restava fermo dopo «Espello la chiavetta…»).

* Wed Nov 18 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.7-3
- Espelli funziona anche con la cartella della chiavetta aperta: RooTree torna
  alla Memoria interna e la espelle un attimo dopo, quando ha finito di leggerla
  (prima diceva che era occupata e non si capiva perché).
- Se la chiavetta è davvero in uso, il messaggio dice chi la usa: un'altra app
  (da chiudere) oppure RooTree stessa che sta ancora preparando le anteprime.

* Wed Nov 18 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.7-2
- Anche il programma si installa in /home/rootree, insieme al suo Qt: sulla
  partizione di sistema, che sui telefoni è piccola, restano meno di 8 MB
  (avvio, modalità root, icona).

* Wed Nov 18 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.7-1
- Un pacchetto solo, completo in sé: Qt 6.12 è dentro RooTree (in
  /home/rootree/qt6) e non serve più installare rootgpt-qt6-runtime, che
  all'aggiornamento viene tolto da solo.
- Chiavette USB: Sailfish OS non le monta da solo; ora compaiono fra le
  Altre memorie come «Tocca per montare» e si montano con un tocco.
- Espelli (⏏) accanto a schede e chiavette: scrive quello che resta in
  sospeso, le smonta e spegne la chiavetta, come su Linux; se qualcosa la
  tiene occupata, RooTree dice cosa.
- Se il sistema non trova memoria libera per montare un exFAT, in modalità
  root RooTree la libera da solo e riprova.

* Tue Nov 17 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.6-3
- Modalità root: svuotare il cestino o eliminare un suo elemento con dentro
  file di root dava «non è un cestino»; ora funziona.

* Tue Nov 17 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.6-2
- Cestino: se un elemento contiene file di root, eliminarlo o svuotare il
  cestino non lo cancella più a metà. Senza modalità root RooTree lo lascia
  intero, toglie tutto il resto e dice che serve la modalità root; con la
  modalità root lo elimina del tutto.

* Tue Nov 17 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.6-1
- Un pacchetto in meno: la modalità root è ora dentro l'app (prima serviva
  anche harbour-rootree-root, da aggiornare sempre insieme). Chi l'aveva lo
  vede tolto da solo all'aggiornamento.
- Modalità root: una cartella di root dentro una cartella dell'utente (per
  esempio nella home) ora va nel cestino; prima l'eliminazione veniva rifiutata.
  Senza modalità root, RooTree avvisa prima che serve.

* Tue Nov 17 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.5-5
- Nessun cambiamento nell'app: il pacchetto porta gli avvisi di licenza
  completi di tutti i componenti di terze parti (NOTICE.md), comprese le
  librerie libssh2 e zstd incluse nel programma.
- Di nuovo anche il pacchetto per armv7hl (telefoni a 32 bit), non ancora
  provato su un telefono vero.

* Mon Nov 16 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.5-4
- Scelta delle cartelle per le sincronizzazioni: c'è «Nuova cartella», anche sul server
  remoto; creata, ci si entra subito e la si può usare

* Mon Nov 16 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.5-3
- Sincronizzazioni: «Sfoglia…» sulla cartella remota apriva per sbaglio la memoria del telefono
  (col nome del server in alto); ora apre davvero le cartelle del server, e una cartella del
  telefono non può più finire come cartella remota

* Mon Nov 16 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.5-2
- Sincronizzazioni, direzione spiegata meglio: Bivalente (telefono ⇄ remoto), Da Locale
  (telefono → remoto) e Da Remoto (remoto → telefono), ognuna col suo verso disegnato, chi comanda
  e a cosa serve; il riquadro «Cosa succederà» dice caso per caso cosa capita se aggiungi,
  modifichi o elimini un file da una parte o dall'altra, con le scelte fatte

* Mon Nov 16 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.5-1
- Sincronizzazioni: una cartella del telefono (memoria interna, Android o scheda SD, con o senza
  sottocartelle) tenuta allineata con una cartella di un NAS, di un server o di un cloud. Si crea
  con una procedura guidata (menù ⋮ → Opzioni cartella → «Sincronizza questa cartella…», oppure
  Opzioni per Remoto → «Sincronizzazioni…»): direzione ed eliminazioni si scelgono per ognuna
- Avviene solo con RooTree aperta: all'apertura, quando un file della cartella cambia, e con
  «Sincronizza ora»; se il remoto non risponde compare un avviso e si riprova al giro dopo
- Salvaguardie: ciò che si elimina va sempre in un cestino; oltre 50 eliminazioni in un giro, o
  con una cartella vuota tutta insieme, RooTree chiede prima di fare qualsiasi cosa; un file
  cambiato dalle due parti non si perde (vince il più recente, l'altro resta)
- Menù ⋮ più corto: «Visualizzazione» (vista e ordinamento) e «Opzioni cartella» sono tendine
- Le nuvole di Mega e di kDrive hanno la loro lettera (M e K)

* Sun Nov 15 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.4-1
- Nuova connessione kDrive (Infomaniak), anche con kSuite gratuita: si collega con un token
  personale creato nel Manager di Infomaniak (permesso «drive») e l'indirizzo del proprio kDrive;
  cartelle, scaricare, caricare, rinominare, spostare e il cestino di kDrive
- I cloud non mostrano più l'account: nel menù delle memorie compare il nome scelto e, accanto,
  il servizio (Dropbox, Megasync, kDrive); nuova voce «Rinomina» nel menù di ogni connessione

* Sun Nov 15 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.3-1
- Nel menù delle memorie lo spazio libero insieme al totale («146 GB liberi su 191 GB»)
- Nuovo tasto «i» su ogni memoria: una pagina con lo spazio diviso per tipo di file (immagini,
  video, musica, documenti, archivi, pacchetti, app, cache, sistema) o per cartella, i file più
  grandi di ogni tipo, i dati del file system (cifratura, spazio riservato, TRIM), la memoria fisica
  (tipo, modello, usura) e lo stato (errori, ultimo controllo)
- Controllo e riparazione delle schede e chiavette ext4 o FAT (con la modalità root); per le schede
  exFAT la pagina spiega come controllarle da un computer

* Sat Nov 14 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.2-1
- Versione 0.9.2: stesse funzioni della 0.9.1 (file del telefono, anteprime, archivi, modalità root,
  cestino, editor di testo, connessioni SFTP, FTP/FTPS, WebDAV/Nextcloud, SMB, S3, Dropbox e Mega)

* Sat Nov 14 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.1-1
- Versione 0.9.1: stesse funzioni della 0.9.0 (file del telefono, anteprime, archivi, modalità root,
  cestino, editor di testo, connessioni SFTP, FTP/FTPS, WebDAV/Nextcloud, SMB, S3, Dropbox e Mega)

* Sat Nov 14 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.9.0-1
- Versione 0.9: tutte le connessioni remote del piano sono pronte (SFTP, FTP/FTPS, WebDAV/Nextcloud,
  SMB, S3, Dropbox, Mega); stesse funzioni della 0.8.0

* Sat Nov 14 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.8.0-1
- Connessioni S3: Amazon S3 e i servizi compatibili (MinIO, Garage, lo S3 dei NAS, Backblaze B2,
  Wasabi, Cloudflare R2). Con un bucket si parte da lì, senza bucket dall'elenco dei bucket
  (si possono anche creare ed eliminare). Cartelle, rinomina, copia e spostamento come sugli altri
  server; file grandi caricati a pezzi; Annulla non lascia resti né pezzi da pagare; la regione si
  trova da sola; il cestino di RooTree anche su S3

* Thu Nov 12 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.1-6
- Editor: la selezione si allarga alle righe sotto, trascinando la maniglia o tenendo premuto e scorrendo
  col dito; la barra Tutto/Copia/Taglia/Incolla è agganciata in fondo e non copre più il testo;
  la maniglia di fine resta sempre in vista

* Thu Nov 12 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.1-5
- Editor come un editor: un tocco mette il cursore lì e toglie la selezione, anche col dito che trema;
  doppio tocco o pressione lunga = parola; la parola in composizione si conferma prima di spostarsi;
  «Modifica» non salta più in fondo al file; le maniglie non si prendono i tocchi della riga sotto;
  tolto il pulsante «Seleziona tutto» in alto (c'è la pillola Tutto)

* Thu Nov 12 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.1-4
- Editor: di nuovo si scrive (la pressione lunga per selezionare vale solo in lettura)

* Thu Nov 12 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.1-3
- Visore ed editor di testo: pressione lunga o doppio tocco seleziona la parola, maniglie per allargare,
  «Seleziona tutto» nella barra, fila Tutto/Copia/Taglia/Incolla

* Thu Nov 12 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.1-2
- Cestino: lo stesso filesystem della home va nel cestino della home anche fuori dalla home
- Cestino: «Svuota il cestino» sempre in vista in cima alla pagina (grigio se è vuoto)

* Thu Nov 12 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.1-1
- Cestino in ogni memoria: interna, Android, SD/USB (formato freedesktop),
  root (.Trash-0), server SFTP/FTP/SMB/WebDAV (.Trash-rootree nella cartella
  iniziale), Nextcloud, Mega e Dropbox (il loro cestino). Elimina = nel
  cestino, con la casella «Elimina definitivamente»; svuotamento da solo
  dopo 30 giorni (7/30/90/mai).
- Menù: «Nuovo file» sotto «Nuova cartella», «Cestino» nella sezione
  Cartella; «Opzioni per Remoto» in una sezione sua, fuori da «Ordina per».

* Tue Nov 10 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.0-2
- I messaggi brevi (es. «Non salvato: accesso negato» dall'editor) ora si vedono anche sopra l'editor e sopra la tastiera

* Tue Nov 10 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.7.0-1
- SMB 2 e 3: le cartelle condivise di Windows, Samba e dei NAS, con utente e password o come ospite; firma e cifratura SMB3
- Senza condivisione nella connessione si parte dall'elenco di quelle del server
- Se il server si riavvia o il Wi-Fi cambia, il collegamento si riprende da solo
- Nuova connessione: i pulsanti restano sopra la tastiera, e un tocco fuori dalle caselle (o Invio) la chiude

* Tue Nov 10 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.9-2
- Editor di testo: l'ultima parola scritta prima di toccare ✓ ora viene salvata (la tastiera la teneva ancora in sospeso)

* Tue Nov 10 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.9-1
- FTP e FTPS (esplicito e implicito sulla porta 990), anche con accesso anonimo
- Certificati fatti in casa dei NAS: la prima volta si mostra l'impronta e si chiede se fidarsi, poi un cambio viene rifiutato
- Si può sfogliare il server mentre una copia è in corso; i file piccoli si copiano in fretta (collegamenti riusati)

* Mon Nov 09 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.8-6
- WebDAV e Nextcloud: la procedura con la password per app e l'avviso sul TOTP, percorso copiabile con un tocco

* Mon Nov 09 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.8-5
- Il cestino anche in Opzioni per Remoto

* Mon Nov 09 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.8-4
- Nextcloud: il cestino, con Ripristina, Elimina definitivamente e Svuota

* Mon Nov 09 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.8-3
- Eliminare e copiare cartelle su WebDAV e cloud con molte meno richieste

* Mon Nov 09 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.8-2
- Nextcloud: icona propria e nome proposto «Nextcloud» al posto dell'indirizzo

* Mon Nov 09 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.8-1
- WebDAV e Nextcloud/ownCloud: sfoglia, copia, rinomina, elimina (Fase 5, step 5)

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.7-4
- Connections list: the detail is hidden when the name already says it.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.7-3
- Mega: optional "don't ask for the code again". The session is kept in the
  password vault; Remote options can revoke it, also on the Mega servers.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.7-2
- Mega: accounts with two-step verification work. The 6-digit code is asked
  at each connection and never saved.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.7-1
- Mega joins Dropbox: sign in with email and password (no app to register),
  browse, copy to and from, rename. Deleting and overwriting move the old
  file to the Mega rubbish bin. Two-step verification is not supported yet.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.5-1
- OneDrive removed: registering an app on the Microsoft portal is too
  complicated to ask of every user.
- Dropbox instructions: they now list exactly which permission boxes to tick
  and warn against the team permissions, which make Dropbox answer "Sign in
  with a work account".

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.4-2
- Connection wizard: the developer sites in the instructions (Dropbox App
  Console, Microsoft Entra / Azure portal) are tappable links that open the
  browser.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.4-1
- OneDrive (personal and work accounts) joins Dropbox: sign-in in the
  browser, browse, copy to and from, rename, delete. Big files are uploaded
  in 10 MiB pieces while being copied. Needs your own free application ID
  from the Microsoft portal (the wizard explains how).

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.3-1
- Phase 5, step 4 (clouds, one at a time): Dropbox. Sign-in happens in the
  browser (OAuth with PKCE, return to 127.0.0.1: the password never goes
  through RooTree); the sign-in is kept in the password vault. Dropbox
  folders are browsed, copied to and from, renamed and deleted like the
  other connections; big files are uploaded in 8 MB pieces while being
  written, so the progress is real. Needs your own free Dropbox App key
  (the wizard explains how). OneDrive and Mega: coming.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.2-3
- Copying a folder where a folder with the same name exists now asks, like
  Dolphin: Merge, Keep both, Skip or Cancel, with "Do this for all folders"
  (separate from the files' choice). Subfolders of a merged folder are
  merged without asking again.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.2-2
- A progress card appears as soon as an operation starts: what it is doing,
  the current file, a thick progress bar (a moving wave while preparing),
  percentage, speed and time left, Cancel; tapping it opens the operations
  list.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.2-1
- Phase 5, step 3: files move between the phone and SFTP servers with the
  same queue as local operations (progress, Cancel, conflicts): copy, cut
  and paste in both directions and on the server, delete, rename, new
  folder and permissions on the server. Interrupted transfers leave no
  partial files behind.
- Tapping a remote file opens it: text files are read and edited directly
  on the server (saved without ever leaving them half written), the rest is
  downloaded to the cache and opened. Share works from servers too.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-6
- Editing an open connection now takes effect: a new start folder is used
  at once, and a changed address, port, user, key or sign-in method closes
  the connection so the next tap reconnects with the new data.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-5
- The connections file is reloaded when it changes on disk, instead of
  being overwritten later with the stale copy kept in memory.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-4
- "Remote options" submenu in the menu: start folder, edit the connection
  and disconnect (when inside a server; then it opens by itself), add a
  connection and lock the password vault.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-3
- In the start folder of a server the menu row is no longer greyed out: it
  shows a check mark and explains how to change it.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-2
- "Always open from this folder" in the menu while browsing a server makes
  the current folder the connection's start folder; the wizard summary has
  the start folder field too (servers imported from ~/.ssh/config skip the
  Server page, so they had no way to set it).

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-1
- Phase 5, step 2: SFTP connections work. Tapping a saved connection
  connects (password or SSH key, also with a passphrase) and opens its
  start folder, browsable like a local one (read-only for now: copying
  comes in the next step). The server fingerprint is checked against
  ~/.ssh/known_hosts like ssh does: a new server asks for confirmation, a
  changed one is refused. "Disconnect" in the connection menu.
- Password vault: passwords can be remembered, encrypted with a master
  password of your choice (Argon2id + XChaCha20-Poly1305), opened once per
  session; "Lock the password vault" in the menu.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.0-3
- Connection wizard: the servers defined in ~/.ssh/config (if it exists)
  are offered at the top of the first step; tapping one fills in an SFTP
  connection (address, port, user, key) and goes to the summary.
- SSH keys are shown as cards with their type, comment and the
  ~/.ssh/config servers that use them; with more than one key none is
  preselected, so the right one is chosen on purpose.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.0-2
- SSH key in the connection wizard: besides typing the path (also with ~/),
  "Browse…" opens a file chooser (hidden files shown, private keys marked),
  and the private keys found in ~/.ssh, recognised by their content, are
  offered as buttons. Warns about a public key or a file that is not a key.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.0-1
- Phase 5, step 1: remote connections interface. A "Network and cloud"
  section in the storage menu lists the saved connections; "Add a
  connection…" opens a step-by-step wizard (type, server, sign-in, name)
  for SFTP, FTP/FTPS, WebDAV, Nextcloud/ownCloud, SMB, S3,
  OneDrive, Dropbox and Mega. Connections can be edited, duplicated and
  deleted. Passwords are never written to the profiles file; connecting
  comes in the next step.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.5.0-1
- Root mode (new harbour-rootree-root package): turned on from the menu with
  the developer password, it starts a separate helper through devel-su; the
  app never runs as root. A red band shows while it is on; it stops when
  turned off or when RooTree closes.
- In root mode: browse, copy, move, delete, rename and create anywhere;
  change permissions, owner and group. Files created as root inside a folder
  owned by the user stay the user's.
- The text viewer becomes an editor ("Edit"): saving never leaves a file
  half written, and as root the original is kept in
  /home/debugging/harbour-rootree (root only, latest copy per file).

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.4.0-3
- Rename / New folder: two selection handles to pick with a finger which
  part of the name to select (double tap selects a word).

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.4.0-2
- Rename / New folder: buttons to select all (or the name without its
  extension), copy and paste; the selection stays while tapping them.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.4.0-1
- Search from the current folder down: by name (also with * and ?), by
  regular expression, or inside the files' content, with results arriving
  while it runs.
- Archives: zip, tar (.gz .bz2 .xz .zst) and 7z open like folders, and
  copying from inside one extracts just those items; "Extract here" and
  "Compress…" in the selection menu. RAR and other read-only formats through
  the system libarchive. Unsafe archive paths are never written outside.
- Sorting by date now shows the newest first; a new "Only for this folder"
  switch (on by default) keeps the chosen sorting for the current folder
  only, while turned off it applies everywhere.

* Tue Oct 27 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.3.1-2
- Classic SailfishOS theme: file icons are tinted too, in the secondary
  highlight colour of the ambience.

* Tue Oct 27 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.3.1-1
- New theme choice in the menu: RooTundra (steel, default) or classic
  SailfishOS (flat, colours and font of the current ambience).
- Every storage in the places menu can be expanded to its standard folders
  (Documents, Downloads, Pictures, Music, Videos); the menu always opens
  collapsed.
- The drop-down arrow next to the storage button is no longer covered.

* Sun Oct 25 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.3.0-1
- Phase 2, open and view: previews of images (also WebP and TIFF), videos
  and PDFs, kept in the standard ~/.cache/thumbnails; built-in image viewer
  (swipe, pinch and double tap zoom) and text viewer.
- Open with the Sailfish apps (default app, "Open with" with the app icons,
  "always use for this type") and share with the system share dialog.
- Files outside the folders that sandboxed Sailfish apps can see are handed
  over through a hard link in ~/Downloads/.rootree, removed after a day.

* Sun Oct 25 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.2.0-1
- Phase 1, local file browser in the style of Dolphin: storage drop-down
  menu (internal, Android, SD card/USB, file system, bookmarks), breadcrumb
  path, back/forward/up, icon and details views, sorting, hidden files.
- New folder, rename, delete, properties with permissions, multiple
  selection, copy/cut/paste with a queue, progress, cancel, conflict and
  error questions.
- RooTundra look: steel panels and buttons, electric blue LEDs, ice and
  frost, steel file icons, Rajdhani font.

* Sun Oct 25 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.1.0-1
- First build (phase 0): lists a local folder through the Rust core.
