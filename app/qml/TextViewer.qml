import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Il visore del testo, che diventa editor con «Modifica»: a capo automatico
// o no, i file troppo grandi si mostrano tagliati (con l'avviso, e allora non
// si modificano: si salverebbe un file troncato), quelli binari no. Si salva
// senza mai lasciare il file a metà; da root l'originale va nelle copie di
// sicurezza (Fase 4: il criterio è modificare un file di /etc).
Popup {
    id: viewer
    property string path
    property var content: ({})
    property bool wrap: true
    property bool editing: false
    property bool dirty: false
    // dove si era leggendo: passando a «Modifica» Qt porta il cursore in
    // fondo al file (setReadOnly) e la pagina scorre fino alla fine; si torna
    // qui (visto il 12/11 col banco di prova: il tocco dopo cadeva altrove)
    property int readCursor: 0
    property real readY: 0
    onEditingChanged: if (editing) Qt.callLater(keepPlace)
    function keepPlace() {
        flick.contentY = readY
        var r = body.positionToRectangle(readCursor)
        var visible = r.y + body.y >= readY && r.y + body.y + r.height <= readY + flick.height
        body.cursorPosition = visible ? readCursor : body.positionAt(0, readY - body.y + 2)
        flick.contentY = readY
        body.forceActiveFocus()
        Qt.inputMethod.show()
    }
    function showEnd() {
        var r = body.positionToRectangle(body.selectionEnd)
        var bottom = body.y + r.y + r.height + handles.knob * 2
        if (bottom > flick.contentY + flick.height)
            flick.contentY = Math.min(bottom - flick.height, Math.max(0, flick.contentHeight - flick.height))
    }
    readonly property bool canEdit: content.text !== undefined && !content.truncated
                                    && (Fs.isWritable(path) || Fs.rootActive)
    signal openWithRequested(string path)
    signal message(string text, bool isError)
    signal saved()

    parent: Overlay.overlay
    x: 0; y: 0
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    padding: 0
    modal: true
    closePolicy: Popup.NoAutoClose
    background: Rectangle { color: Style.bg }

    function openFor(p) {
        path = p
        content = Opener.readText(p)
        editing = false
        dirty = false
        open()
        flick.contentY = 0
        flick.contentX = 0
    }

    function save() {
        // l'ultima parola scritta è ancora «in composizione» nella tastiera:
        // senza commit() resterebbe fuori dal file (visto il 10/11)
        Qt.inputMethod.commit()
        var r = Fs.writeText(path, body.text)
        if (r.error) {
            message(Style.t("Non salvato: ", "Not saved: ") + r.error, true)
            return false
        }
        dirty = false
        message(r.backup ? Style.t("Salvato. L'originale è in ", "Saved. The original is in ") + r.backup
                         : Style.t("Salvato", "Saved"), false)
        saved()
        return true
    }

    AppDialog {
        id: unsaved
        title: Style.t("Modifiche non salvate", "Unsaved changes")
        message: Style.t("Salvare «" + viewer.path.split("/").pop() + "» prima di chiudere?", "Save “" + viewer.path.split("/").pop() + "” before closing?")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Scarta", "Discard"), role: "danger" }, { text: Style.t("Salva", "Save"), role: "accept" }]
        onChosen: function (role) {
            if (role === "danger") { viewer.dirty = false; viewer.close() }
            else if (role === "accept" && viewer.save()) viewer.close()
        }
    }

    contentItem: Item {
        ViewerBar {
            id: bar
            anchors { left: parent.left; right: parent.right; top: parent.top }
            title: viewer.path.split("/").pop()
            subtitle: viewer.editing ? (viewer.dirty ? Style.t("Modificato, non salvato", "Modified, not saved") : Style.t("Modifica", "Editing"))
                      : viewer.content.truncated ? Style.t("Mostrati i primi 2 MB", "Showing the first 2 MB") : ""
            onCloseClicked: viewer.dirty ? unsaved.open() : viewer.close()
            IconButton { icon: "wrap"; checked: viewer.wrap; onClicked: viewer.wrap = !viewer.wrap }
            IconButton { visible: viewer.canEdit && !viewer.editing; icon: "rename"; onClicked: viewer.editing = true }
            IconButton { visible: viewer.editing; icon: "check"; checked: viewer.dirty; onClicked: viewer.save() }
            IconButton { visible: !viewer.editing; icon: "share"; onClicked: Opener.share([viewer.path]) }
            IconButton { visible: !viewer.editing; icon: "openwith"; onClicked: viewer.openWithRequested(viewer.path) }
        }
        Flickable {
            id: flick
            objectName: "flick"
            // finisce sopra la barra della selezione (o sopra la tastiera): la
            // barra non copre mai il testo (prima galleggiava sulle ultime
            // righe e la sua ✕ si prendeva i tocchi delle maniglie, 12/11)
            anchors { left: parent.left; right: parent.right; top: bar.bottom; bottom: selBar.visible ? selBar.top : parent.bottom }
            anchors.bottomMargin: selBar.visible ? 0 : Style.keyboardHeight(viewer.height, viewer.width)
            contentWidth: viewer.wrap ? width : Math.max(width, body.implicitWidth + 2 * Style.margin)
            // spazio in fondo per il disco della maniglia sotto l'ultima riga
            contentHeight: body.implicitHeight + 2 * Style.margin + handles.knob * 2
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            visible: viewer.content.text !== undefined
            onContentYChanged: if (!viewer.editing) viewer.readY = contentY
            TextEdit {
                id: body
                objectName: "body"
                x: Style.margin
                y: Style.margin
                width: viewer.wrap ? flick.width - 2 * Style.margin : implicitWidth
                readOnly: !viewer.editing
                // i tocchi li gestisce tutti `touch` qui sotto: il TextEdit da
                // solo, dentro la Flickable, perdeva i tocchi appena il dito
                // tremava (li prendeva lo scorrimento)
                selectByMouse: false
                activeFocusOnPress: false
                persistentSelection: true
                selectionColor: Style.accent
                selectedTextColor: "#ffffff"
                onTextChanged: if (viewer.editing) viewer.dirty = true
                onCursorPositionChanged: if (!viewer.editing) viewer.readCursor = cursorPosition
                // la maniglia di fine (un disco sotto la riga) resta in vista:
                // sull'ultima riga visibile finiva sotto la barra (12/11)
                onSelectionEndChanged: if (selectedText.length > 0) Qt.callLater(viewer.showEnd)
                // il cursore resta in vista mentre si scrive
                onCursorRectangleChanged: {
                    if (!viewer.editing) return
                    var r = cursorRectangle
                    if (r.y + y < flick.contentY) flick.contentY = r.y + y - Style.margin
                    else if (r.y + y + r.height > flick.contentY + flick.height) flick.contentY = r.y + y + r.height - flick.height + Style.margin
                }
                textFormat: TextEdit.PlainText
                wrapMode: viewer.wrap ? TextEdit.WrapAtWordBoundaryOrAnywhere : TextEdit.NoWrap
                text: viewer.content.text || ""
                color: Style.text
                font.family: "monospace"
                font.pixelSize: 12.5 * Style.dp
            }
            // Tutti i tocchi sul testo, come in un editor qualunque (richiesta
            // dell'utente del 12/11): un tocco mette il cursore lì e toglie la
            // selezione; doppio tocco o pressione lunga = la parola. Una
            // MouseArea (non un TapHandler): si prende il tocco intero, e la
            // Flickable lo ruba solo se il dito scorre davvero.
            MouseArea {
                id: touch
                objectName: "touch"
                width: flick.contentWidth
                height: Math.max(flick.contentHeight, flick.height)
                z: 2
                function posAt(m) {
                    var p = mapToItem(body, m.x, m.y)
                    return body.positionAt(p.x, p.y)
                }
                function place(m) {
                    // la parola ancora «in composizione» va scritta prima di
                    // spostarsi, sennò il tocco finisce dentro la tastiera
                    // (l'ultima parola scritta non prendeva il cursore)
                    Qt.inputMethod.commit()
                    var pos = posAt(m)
                    body.deselect()
                    body.cursorPosition = pos
                    if (viewer.editing) {
                        body.forceActiveFocus()
                        Qt.inputMethod.show()
                    }
                }
                function word(m) {
                    Qt.inputMethod.commit()
                    body.cursorPosition = posAt(m)
                    body.selectWord()
                    if (viewer.editing) body.forceActiveFocus()
                }
                // pressione lunga e poi, senza alzare il dito, si allarga la
                // selezione (come su Android): finché si tiene, niente scorrimento
                property bool extending: false
                property int anchorStart: 0
                property int anchorEnd: 0
                onClicked: function (m) { place(m) }
                onDoubleClicked: function (m) { word(m) }
                onPressAndHold: function (m) {
                    word(m)
                    anchorStart = body.selectionStart
                    anchorEnd = body.selectionEnd
                    extending = true
                    preventStealing = true
                }
                onPositionChanged: function (m) {
                    if (!extending) return
                    var pos = posAt(m)
                    if (pos < anchorStart) body.select(anchorEnd, pos)
                    else body.select(anchorStart, Math.max(pos, anchorEnd))
                }
                onReleased: { extending = false; preventStealing = false }
                onCanceled: { extending = false; preventStealing = false }
            }
            // dentro il contenuto: scorrono insieme al testo, sopra i tocchi
            SelectionHandles { id: handles; field: body; z: 5; onTapped: function (x, y) { touch.place({ x: x, y: y }) } }
        }
        // cosa fare con la selezione: Tutto, Copia; in modifica anche Taglia e Incolla
        Rectangle {
            id: selBar
            readonly property bool has: body.selectedText.length > 0
            // in modifica sempre (c'è «Tutto», al posto del pulsante in alto tolto il 12/11)
            visible: has || viewer.editing
            // una barra agganciata in fondo, sopra la tastiera
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; bottomMargin: Style.keyboardHeight(viewer.height, viewer.width) }
            height: selRow.implicitHeight + 12 * Style.dp
            color: Style.bg2
            Rectangle { anchors { left: parent.left; right: parent.right; top: parent.top } height: 1 * Style.dp; color: Style.line }
            Row {
                id: selRow
                anchors.centerIn: parent
                spacing: 6 * Style.dp
                SteelPill { text: Style.t("Tutto", "All"); onClicked: body.selectAll() }
                SteelPill {
                    visible: selBar.has
                    text: Style.t("Copia", "Copy")
                    onClicked: { body.copy(); viewer.message(Style.t("Copiato", "Copied"), false) }
                }
                SteelPill {
                    visible: selBar.has && viewer.editing
                    text: Style.t("Taglia", "Cut")
                    onClicked: body.cut()
                }
                SteelPill {
                    visible: viewer.editing && body.canPaste
                    text: Style.t("Incolla", "Paste")
                    onClicked: { body.forceActiveFocus(); body.paste() }
                }
                SteelPill {
                    visible: selBar.has
                    text: "✕"
                    onClicked: body.deselect()
                }
            }
        }
        Column {
            visible: viewer.content.text === undefined
            anchors.centerIn: parent
            width: parent.width * 0.8
            spacing: 12 * Style.dp
            Text {
                font.family: Style.fontFamily
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: viewer.content.binary ? Style.t("Non è un file di testo.", "This is not a text file.")
                                            : (viewer.content.error || "")
                color: Style.text
                font.pixelSize: Style.fontL
            }
            SteelPill {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Style.t("Apri con…", "Open with…")
                onClicked: viewer.openWithRequested(viewer.path)
            }
        }
    }
}
