# Banco di prova dell'editor di testo, sul PC, con tocchi veri

PySide6 (Qt 6.11 di Fedora) carica `app/qml/TextViewer.qml` con `Fs` e `Opener`
finti (`stub/RooTree`) e lo guida con eventi touch di QTest: pressione lunga,
doppio tocco, tocco con il dito che trema, trascinamento, scrittura come la
tastiera (QInputMethodEvent con commit). Niente telefono, niente schermo.

    cd tools/collaudo-editor && ln -sfn ../../app/qml app
    QT_QPA_PLATFORM=offscreen python3 -I prova.py     # «TUTTO OK»

Il 12/11/2026 ha trovato: le maniglie della selezione si prendevano i tocchi
della riga sotto; passando a «Modifica» Qt porta il cursore in fondo al file
(setReadOnly) e la pagina scorreva fino alla fine. Non prova la parola «in
composizione» della tastiera di Sailfish (sul PC non c'è un metodo
d'immissione): quella si vede solo sul telefono.
