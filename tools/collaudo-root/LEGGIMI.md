# Collaudo della modalità root senza toccare lo schermo

`qml/Main.qml` è un QML senza finestre (radice Item) che chiama le stesse
funzioni dei pulsanti: password sbagliata, password giusta, elenco di /root,
salvataggio di `/etc/rootree-prova.conf` con copia di sicurezza, cambio di
gruppo, copia da root nella cartella di lavoro (deve restare dell'utente),
spegnimento. Si usa col binario vero:

1. sul telefono, in una cartella di lavoro `/home/debugging/AAAA-MM-GG-…`:
   `harbour-rootree` e `qt.conf` copiati da `/usr/share/harbour-rootree/`,
   accanto a questa `qml/`; `/etc/rootree-prova.conf` creato con sudo;
2. la password di sviluppatore in `pw` (umask 077), da togliere subito dopo;
3. ambiente della sessione (`systemctl --user show-environment`) più quello
   del launcher (LD_LIBRARY_PATH col runtime, QT_PLUGIN_PATH,
   QML2_IMPORT_PATH, `QT_QPA_PLATFORM=wayland-egl`, LANG UTF-8,
   `QT_FORCE_STDERR_LOGGING=1`), poi `./harbour-rootree`.

⛔ Il runtime Qt non ha il plugin `offscreen`; e il launcher impone
`wayland-egl`, quindi `QT_QPA_PLATFORM=offscreen` davanti a
`/usr/bin/harbour-rootree` non fa niente.

Il nucleo da solo si prova con `rust/examples/prova-root.rs` (password da
stdin).
