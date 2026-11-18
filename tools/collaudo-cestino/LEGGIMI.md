# Collaudo del cestino senza toccare lo schermo

Come `tools/collaudo-root`: binario vero copiato da `/home/rootree/app/ (fino alla 0.9.7-1: /usr/share/harbour-rootree/)`
(con `qt.conf`) in una cartella di lavoro `/home/debugging/AAAA-MM-GG-…`,
accanto alla `qml/` di prova, con l'ambiente della sessione e del launcher.

- `locale/`: argomenti = cartelle di prova, una per memoria (es. una in
  `/home/debugging/…`, `~/android_storage/.rootree-prova-cestino`, una sulla SD).
  Per ognuna: Nuova cartella / Nuovo file (anche il doppione che deve fallire),
  nel cestino, elenco, ripristino, definitivo dal cestino, eliminazione
  definitiva della cartella ripristinata.
- `root/`: argomento = cartella di lavoro con `pw` (password di sviluppatore,
  umask 077, da togliere subito). Prima `/etc/rootree-prova-cestino.conf`
  creato con sudo; finisce eliminato per sempre dal cestino di root (`/.Trash-0`).

Visto il 12/11/2026 sul POCO (0.7.1-2): tutto riuscito; dopo, togliere i
`.Trash-*` vuoti creati dalla prova.
