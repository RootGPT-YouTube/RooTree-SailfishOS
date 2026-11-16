# Banco di prova delle sincronizzazioni e del menù a tendine, sul PC

PySide6 (Qt 6.11 di Fedora) carica il `app/qml/Main.qml` **vero** con i
singleton C++ finti di `stub/RooTree` (Fs, Jobs, Remotes, Syncs, Opener,
FolderModel, SearchModel) e lo guida con eventi touch di QTest: apre il menù ⋮,
le tendine «Visualizzazione» e «Opzioni cartella», fa tutta la procedura
«Nuova sincronizzazione» (cassaforte, selettore della cartella remota, regole,
nome), poi finge gli esiti del motore (conferma «cartella remota vuota», esito
con i numeri, server irraggiungibile con l'avviso che sparisce in 5 s) e prova
l'elenco (Sincronizza ora, Modifica, Elimina). Per toccare una riga sotto il
bordo scorre il pannello col dito, come una persona.

    QT_QPA_PLATFORM=offscreen python3 -I tools/collaudo-sync/prova.py   # «TUTTO OK»

Con `BANCO_SHOTS=<cartella>` salva gli scatti dei passi; con `BANCO_LOG=1`
stampa i messaggi del QML. Le impostazioni (Settings) vanno in una cartella
temporanea, mai in quelle vere. Il motore (sync.rs) si prova a parte con
`cargo test sincronizzazione`.
