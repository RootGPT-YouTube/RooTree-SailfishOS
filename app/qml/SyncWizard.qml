import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// La procedura per una sincronizzazione (0.9.5, richiesta dell'utente del
// 15/11), sullo stile di «Nuova connessione»: Cartella del telefono →
// Connessione → Cartella remota → Regole → Nome e riepilogo. Si salva in
// Syncs; il primo giro lo fa partire chi la apre (Main.qml, onSaved).
Popup {
    id: wiz
    signal saved(string id)
    // la connessione serve aperta per sfogliare le sue cartelle: la apre
    // Main.qml (cassaforte, password…) e poi chiama `then`
    signal connectRequested(string remoteId, var then)

    parent: Overlay.overlay
    x: 0; y: 0
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    padding: 0
    modal: true
    closePolicy: Popup.NoAutoClose
    background: Rectangle {
        color: Style.bg
        Rectangle {
            anchors.fill: parent
            visible: !Style.sailfish
            gradient: Gradient {
                GradientStop { position: 0; color: Style.bg2 }
                GradientStop { position: 1; color: Style.bg }
            }
        }
    }

    // ── stato ──────────────────────────────────────────────────────────────
    property var s: ({})
    property bool editing: false
    property int page: 0
    // il nome l'ha scritto l'utente: non si ripropone più
    property bool nameTouched: false
    readonly property var pages: [Style.t("Telefono", "Phone"), Style.t("Remoto", "Remote"), Style.t("Cartella", "Folder"), Style.t("Regole", "Rules"), Style.t("Nome", "Name")]
    readonly property var conn: s.remote ? Remotes.get(s.remote) : ({})
    readonly property string connName: conn.name || Style.t("remoto", "remote")

    function set(key, value) { var c = Object.assign({}, s); c[key] = value; s = c; if (key !== "name" && !nameTouched && !editing) proposeName() }
    function v(key) { return s[key] === undefined ? "" : s[key] }
    function lastName(p) { return (p || "").replace(/\/+$/, "").split("/").pop() || "/" }
    // «Foto → NAS Remoto»
    function proposeName() {
        if (!s.local || !s.remote) return
        var c = Object.assign({}, s); c.name = lastName(s.local) + (s.direction === "down" ? " ← " : s.direction === "both" ? " ⇄ " : " → ") + connName; s = c
    }
    // il percorso del telefono come lo dice l'utente: «Memoria interna/Pictures»
    function localLabel(p) {
        if (!p) return ""
        if (p.indexOf(Fs.home + "/android_storage") === 0) return Style.t("Memoria Android", "Android storage") + p.substring((Fs.home + "/android_storage").length)
        if (p.indexOf(Fs.home) === 0) return Style.t("Memoria interna", "Internal storage") + p.substring(Fs.home.length)
        var m = p.match(/^\/run\/media\/[^\/]+\/[^\/]+(.*)$/)
        if (m) return Style.t("Scheda SD", "SD card") + m[1]
        return p
    }

    function openNew(localDir) {
        editing = false
        nameTouched = false
        s = { local: localDir || "", remote: "", remotePath: "", subdirs: true, hidden: false, direction: "up", deletions: "none", enabled: true, name: "" }
        page = 0
        syncChecks()
        open()
    }
    // le caselle perdono il legame col primo tocco: a ogni apertura si rimettono
    function syncChecks() {
        subdirsCheck.checked = s.subdirs === true
        hiddenCheck.checked = s.hidden === true
        enabledCheck.checked = s.enabled !== false
    }
    function openFor(sync) {
        editing = true
        nameTouched = true
        s = Object.assign({}, sync)
        page = 0
        syncChecks()
        open()
    }
    readonly property bool pageOk: page === 0 ? v("local") !== ""
                                  : page === 1 ? v("remote") !== "" && conn.id !== undefined
                                  : page === 4 ? v("name").trim() !== ""
                                  : true
    function next() {
        hideKeyboard()
        // la cartella remota proposta: quella del telefono con lo stesso nome
        if (page === 1 && v("remotePath") === "") set("remotePath", "/" + lastName(v("local")))
        if (page < 4) page++
    }
    function back() {
        hideKeyboard()
        if (page === 0) close()
        else page--
    }
    function hideKeyboard() {
        Qt.inputMethod.commit()
        flick.forceActiveFocus()
        Qt.inputMethod.hide()
    }
    function save() {
        hideKeyboard()
        var c = Object.assign({}, s)
        c.name = c.name.trim()
        if (c.remotePath === "") c.remotePath = "/"
        if (c.remotePath.charAt(0) !== "/") c.remotePath = "/" + c.remotePath
        var id = Syncs.save(c)
        close()
        saved(id)
    }
    // dal percorso del selettore (/@remote/<id>/…) al percorso sul server
    function serverPath(p) {
        var pre = "/@remote/" + v("remote")
        return p.indexOf(pre) === 0 ? (p.substring(pre.length) || "/") : p
    }

    // ── componenti ─────────────────────────────────────────────────────────
    component Label: Text {
        font.family: Style.fontFamily
        width: parent ? parent.width : 0
        color: Style.dim
        font.pixelSize: Style.fontS
        wrapMode: Text.Wrap
    }
    component Title: Text {
        font.family: Style.fontFamily
        width: parent ? parent.width : 0
        color: Style.text
        font.pixelSize: Style.fontM
        wrapMode: Text.Wrap
    }
    component Field: TextField {
        id: tf
        property string key
        font.family: Style.fontFamily
        width: parent ? parent.width : 0
        color: Style.text
        font.pixelSize: Style.fontM
        selectByMouse: true
        selectionColor: Style.accent
        placeholderTextColor: Style.dim
        inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
        text: key ? wiz.v(key) : ""
        onTextEdited: if (key) { if (key === "name") wiz.nameTouched = true; wiz.set(key, text) }
        onAccepted: wiz.hideKeyboard()
        background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: tf.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
    }
    // una scelta fra più (direzione, eliminazioni): riquadro con spunta, titolo e spiegazione
    component Choice: Rectangle {
        id: ch
        property string title
        property string note
        property bool chosen: false
        signal picked()
        width: parent ? parent.width : 0
        height: chCol.implicitHeight + 16 * Style.dp
        radius: 12 * Style.dp
        color: chTap.pressed ? Style.pressed : (chosen ? Style.accentSoft : Style.panel)
        border.color: chosen ? Style.accent : Style.line
        border.width: (chosen ? 2 : 1) * Style.dp
        Column {
            id: chCol
            anchors { left: parent.left; right: tick.left; verticalCenter: parent.verticalCenter; leftMargin: Style.margin; rightMargin: 6 * Style.dp }
            spacing: 2 * Style.dp
            Text { font.family: Style.fontFamily; width: parent.width; text: ch.title; color: Style.text; font.pixelSize: Style.fontM; font.bold: ch.chosen; wrapMode: Text.Wrap }
            Text { font.family: Style.fontFamily; width: parent.width; visible: ch.note !== ""; text: ch.note; color: Style.dim; font.pixelSize: Style.fontS; wrapMode: Text.Wrap }
        }
        Icon { id: tick; anchors { right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter } name: "check"; size: 18 * Style.dp; visible: ch.chosen }
        MouseArea { id: chTap; anchors.fill: parent; onClicked: ch.picked() }
    }
    // un riquadro informativo (avvertenze, spiegazioni)
    component Note: Rectangle {
        property alias text: noteText.text
        property bool warn: false
        width: parent ? parent.width : 0
        height: noteText.implicitHeight + 2 * 10 * Style.dp
        radius: 10 * Style.dp
        color: "transparent"
        border.color: warn ? Style.danger : Style.line
        border.width: 1 * Style.dp
        Text {
            id: noteText
            font.family: Style.fontFamily
            x: Style.margin; y: 10 * Style.dp
            width: parent.width - 2 * Style.margin
            color: Style.dim
            font.pixelSize: Style.fontS
            wrapMode: Text.Wrap
            textFormat: Text.StyledText
        }
    }

    FilePicker {
        id: localPicker
        dirsOnly: true
        title: Style.t("Cartella del telefono", "Phone folder")
        onPicked: function (path) { wiz.set("local", path) }
    }
    FilePicker {
        id: remotePicker
        dirsOnly: true
        title: wiz.connName
        onPicked: function (path) { wiz.set("remotePath", wiz.serverPath(path)) }
    }

    contentItem: Item {
        // ── intestazione: titolo e passi ───────────────────────────────────
        SteelPanel {
            id: top
            anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: -border.width; rightMargin: -border.width; topMargin: -border.width }
            height: Style.bar + steps.height + 10 * Style.dp
            ledWidth: width * (wiz.page + 1) / 5
            led: "bottom"
            IconButton { id: closeBtn; anchors { left: parent.left; top: parent.top; topMargin: 2 * Style.dp } icon: "close"; onClicked: wiz.close() }
            Text {
                font.family: Style.fontFamily
                anchors { left: closeBtn.right; right: parent.right; verticalCenter: closeBtn.verticalCenter; rightMargin: Style.margin }
                text: wiz.editing ? Style.t("Modifica sincronizzazione", "Edit sync") : Style.t("Nuova sincronizzazione", "New sync")
                color: Style.text
                font.pixelSize: Style.fontL
                font.bold: true
                elide: Text.ElideRight
            }
            Row {
                id: steps
                anchors { left: parent.left; right: parent.right; top: closeBtn.bottom; leftMargin: Style.margin; rightMargin: Style.margin }
                height: 34 * Style.dp
                Repeater {
                    model: wiz.pages
                    delegate: Item {
                        width: steps.width / 5
                        height: steps.height
                        readonly property bool done: index < wiz.page
                        readonly property bool current: index === wiz.page
                        Rectangle {
                            id: dot
                            anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                            width: 20 * Style.dp; height: width; radius: width / 2
                            color: parent.current ? Style.accent : (parent.done ? Style.accentSoft : "transparent")
                            border.color: Style.accent
                            border.width: 1.5 * Style.dp
                            Text { anchors.centerIn: parent; font.family: Style.fontFamily; text: index + 1; color: Style.text; font.pixelSize: 11 * Style.dp; font.bold: true }
                        }
                        Text {
                            font.family: Style.fontFamily
                            anchors { left: dot.right; leftMargin: 4 * Style.dp; right: parent.right; verticalCenter: parent.verticalCenter }
                            text: modelData
                            color: parent.current ? Style.text : Style.dim
                            font.pixelSize: 11.5 * Style.dp
                            font.bold: parent.current
                            elide: Text.ElideRight
                        }
                    }
                }
            }
        }

        // ── le pagine ──────────────────────────────────────────────────────
        Flickable {
            id: flick
            anchors { left: parent.left; right: parent.right; top: top.bottom; bottom: bottomBar.top }
            contentHeight: pagesCol.implicitHeight + 2 * Style.margin
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            MouseArea {
                width: flick.width
                height: Math.max(flick.height, flick.contentHeight)
                onClicked: wiz.hideKeyboard()
            }
            Column {
                id: pagesCol
                x: Style.margin
                y: Style.margin
                width: flick.width - 2 * Style.margin
                spacing: 10 * Style.dp

                // 1 · Cartella del telefono
                Column {
                    visible: wiz.page === 0
                    width: parent.width
                    spacing: 10 * Style.dp
                    Title { text: Style.t("Quale cartella del telefono vuoi sincronizzare?", "Which phone folder do you want to sync?") }
                    Label { text: Style.t("Dalla memoria interna, dalla memoria Android o dalla scheda SD.", "From internal storage, Android storage or the SD card.") }
                    Rectangle {
                        width: parent.width
                        height: Style.row + 8 * Style.dp
                        radius: 12 * Style.dp
                        color: localTap.pressed ? Style.pressed : Style.panel
                        border.color: wiz.v("local") ? Style.accent : Style.line
                        border.width: 1 * Style.dp
                        Icon { id: lfIcon; anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter } name: "folder" }
                        Column {
                            anchors { left: lfIcon.right; leftMargin: Style.margin; right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                            Text { font.family: Style.fontFamily; width: parent.width; text: wiz.v("local") ? wiz.lastName(wiz.v("local")) : Style.t("Scegli la cartella…", "Choose the folder…"); color: Style.text; font.pixelSize: Style.fontM; font.bold: wiz.v("local") !== ""; elide: Text.ElideRight }
                            Text { font.family: Style.fontFamily; width: parent.width; visible: wiz.v("local") !== ""; text: wiz.localLabel(wiz.v("local")); color: Style.dim; font.pixelSize: Style.fontS; elide: Text.ElideMiddle }
                        }
                        MouseArea { id: localTap; anchors.fill: parent; onClicked: localPicker.openAt(wiz.v("local") || Fs.home) }
                    }
                    CheckRow {
                        id: subdirsCheck
                        text: Style.t("Includi le sottocartelle", "Include subfolders")
                        checked: wiz.s.subdirs === true
                        onCheckedChanged: if (checked !== (wiz.s.subdirs === true)) wiz.set("subdirs", checked)
                    }
                    CheckRow {
                        id: hiddenCheck
                        text: Style.t("Includi i file nascosti", "Include hidden files")
                        checked: wiz.s.hidden === true
                        onCheckedChanged: if (checked !== (wiz.s.hidden === true)) wiz.set("hidden", checked)
                    }
                    Label { text: Style.t("I file nascosti (che iniziano col punto) sono di solito miniature e dati delle app: meglio lasciarli fuori.", "Hidden files (starting with a dot) are usually thumbnails and app data: better leave them out.") }
                }

                // 2 · Connessione
                Column {
                    visible: wiz.page === 1
                    width: parent.width
                    spacing: 8 * Style.dp
                    Title { text: Style.t("Con quale memoria di rete o cloud?", "With which network or cloud storage?") }
                    Label {
                        visible: Remotes.list.length === 0
                        text: Style.t("Non c'è ancora nessuna connessione: aggiungine una dal menù ⋮ → Opzioni per Remoto → «Aggiungi una connessione…», poi torna qui.",
                                      "There is no connection yet: add one from the ⋮ menu → Remote options → “Add a connection…”, then come back here.")
                    }
                    Repeater {
                        model: Remotes.list
                        delegate: Rectangle {
                            width: parent.width
                            height: Style.row + 6 * Style.dp
                            radius: 12 * Style.dp
                            readonly property bool chosen: wiz.v("remote") === modelData.id
                            color: connTap.pressed ? Style.pressed : (chosen ? Style.accentSoft : Style.panel)
                            border.color: chosen ? Style.accent : Style.line
                            border.width: (chosen ? 2 : 1) * Style.dp
                            TypeBadge { id: cb; anchors { left: parent.left; leftMargin: 8 * Style.dp; verticalCenter: parent.verticalCenter } width: 40 * Style.dp; icon: Remotes.typeInfo(modelData.type).icon || "cloud"; lit: parent.chosen }
                            Column {
                                anchors { left: cb.right; leftMargin: Style.margin; right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                                Text { font.family: Style.fontFamily; width: parent.width; text: modelData.name; color: Style.text; font.pixelSize: Style.fontM; font.bold: true; elide: Text.ElideRight }
                                Text { font.family: Style.fontFamily; width: parent.width; text: Remotes.describe(modelData); color: Style.dim; font.pixelSize: Style.fontS; elide: Text.ElideMiddle }
                            }
                            MouseArea {
                                id: connTap
                                anchors.fill: parent
                                onClicked: {
                                    // un'altra connessione: la cartella remota si ripropone
                                    if (wiz.v("remote") !== modelData.id && !wiz.editing) wiz.set("remotePath", "")
                                    wiz.set("remote", modelData.id)
                                }
                            }
                        }
                    }
                }

                // 3 · Cartella remota
                Column {
                    visible: wiz.page === 2
                    width: parent.width
                    spacing: 10 * Style.dp
                    Title { text: Style.t("In quale cartella di «" + wiz.connName + "»?", "Which folder of “" + wiz.connName + "”?") }
                    Field { key: "remotePath"; placeholderText: "/FotoRemoto"; inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText | Qt.ImhUrlCharactersOnly }
                    SteelPill {
                        text: Style.t("Sfoglia…", "Browse…")
                        onClicked: {
                            wiz.hideKeyboard()
                            var id = wiz.v("remote")
                            wiz.connectRequested(id, function () {
                                var base = Remotes.entryPath(id) || ("/@remote/" + id)
                                var want = "/@remote/" + id + (wiz.v("remotePath") || "")
                                remotePicker.openAt(Fs.exists(want) ? want : base)
                            })
                        }
                    }
                    Label { text: Style.t("Il percorso è quello sul server, dalla sua radice. Se la cartella non c'è, al primo giro si crea (quando la sincronizzazione carica dal telefono).",
                                          "The path is the one on the server, from its root. If the folder does not exist, the first run creates it (when the sync uploads from the phone).") }
                }

                // 4 · Regole
                Column {
                    visible: wiz.page === 3
                    width: parent.width
                    spacing: 8 * Style.dp
                    MenuSection { width: parent.width; text: Style.t("Direzione", "Direction") }
                    Choice {
                        title: Style.t("Dal telefono a «" + wiz.connName + "»", "From the phone to “" + wiz.connName + "”")
                        note: Style.t("I file nuovi o cambiati sul telefono si copiano sul remoto. Quelli nuovi sul remoto restano lì.", "New or changed files on the phone are copied to the remote. New files on the remote stay there.")
                        chosen: wiz.v("direction") === "up"
                        onPicked: wiz.set("direction", "up")
                    }
                    Choice {
                        title: Style.t("Da «" + wiz.connName + "» al telefono", "From “" + wiz.connName + "” to the phone")
                        note: Style.t("I file nuovi o cambiati sul remoto si copiano sul telefono.", "New or changed files on the remote are copied to the phone.")
                        chosen: wiz.v("direction") === "down"
                        onPicked: wiz.set("direction", "down")
                    }
                    Choice {
                        title: Style.t("Nei due sensi", "Both ways")
                        note: Style.t("Ogni file nuovo o cambiato va dall'altra parte. Cambiato da tutte e due: vince il più recente, l'altro non si perde.", "Every new or changed file goes to the other side. Changed on both: the newest wins, the other is not lost.")
                        chosen: wiz.v("direction") === "both"
                        onPicked: wiz.set("direction", "both")
                    }
                    MenuSection { width: parent.width; text: Style.t("Eliminazioni", "Deletions") }
                    Choice {
                        title: Style.t("Non ripeterle", "Do not repeat them")
                        note: Style.t("Ciò che elimini da una parte resta dall'altra" + (wiz.v("direction") === "both" ? " (e al giro dopo torna indietro)." : "."), "What you delete on one side stays on the other" + (wiz.v("direction") === "both" ? " (and comes back at the next run)." : "."))
                        chosen: wiz.v("deletions") === "none"
                        onPicked: wiz.set("deletions", "none")
                    }
                    Choice {
                        title: Style.t("Eliminati sul telefono ⇒ anche sul remoto", "Deleted on the phone ⇒ also on the remote")
                        note: Style.t("Sul remoto finiscono nel cestino del servizio (o in quello di RooTree sul server).", "On the remote they go to the service's trash (or RooTree's trash on the server).")
                        chosen: wiz.v("deletions") === "up"
                        onPicked: wiz.set("deletions", "up")
                    }
                    Choice {
                        title: Style.t("Eliminati sul remoto ⇒ anche sul telefono", "Deleted on the remote ⇒ also on the phone")
                        note: Style.t("Per esempio sposti sul NAS le foto in un'altra cartella d'archivio: dal telefono spariscono. Finiscono nel cestino di RooTree: lo spazio si libera quando lo svuoti.", "For example you move the photos on the NAS to another archive folder: they disappear from the phone. They go to RooTree's trash: the space is freed when you empty it.")
                        chosen: wiz.v("deletions") === "down"
                        onPicked: wiz.set("deletions", "down")
                    }
                    Choice {
                        title: Style.t("In entrambi i sensi", "Both ways")
                        chosen: wiz.v("deletions") === "both"
                        onPicked: wiz.set("deletions", "both")
                    }
                    Note {
                        text: Style.t("<b>Salvaguardie.</b> Ciò che si elimina va sempre in un cestino. Se in un giro le eliminazioni sono più di 50, o se una delle due cartelle si presenta vuota tutta insieme (NAS montato male, scheda tolta), RooTree non fa niente e ti chiede prima.",
                                      "<b>Safeguards.</b> What is deleted always goes to a trash. If a run would delete more than 50 files, or one of the two folders suddenly looks empty (NAS badly mounted, card removed), RooTree does nothing and asks you first.")
                    }
                    MenuSection { width: parent.width; text: Style.t("Quando", "When") }
                    Note {
                        text: Style.t("La sincronizzazione avviene <b>solo con RooTree aperta</b>: quando la apri, quando un file della cartella del telefono cambia mentre è aperta, e quando scegli «Sincronizza ora» (menù ⋮ → Opzioni per Remoto). Niente lavori in background, per non pesare sulla batteria. Se la connessione usa la cassaforte delle password, all'apertura RooTree te la chiede.",
                                      "The sync happens <b>only while RooTree is open</b>: when you open it, when a file in the phone folder changes while it is open, and when you choose “Sync now” (⋮ menu → Remote options). No background work, to spare the battery. If the connection uses the password vault, RooTree asks for it at opening.")
                    }
                    Note {
                        visible: wiz.conn.type === "kdrive"
                        warn: true
                        text: Style.t("kDrive non accetta più di circa 55-60 richieste al minuto: con molti file la sincronizzazione è lenta, e non si può fare di meglio.",
                                      "kDrive accepts no more than about 55-60 requests per minute: with many files the sync is slow, and nothing can be done about it.")
                    }
                }

                // 5 · Nome e riepilogo
                Column {
                    visible: wiz.page === 4
                    width: parent.width
                    spacing: 8 * Style.dp
                    Title { text: Style.t("Come la vuoi chiamare?", "What do you want to call it?") }
                    Field { key: "name" }
                    CheckRow {
                        id: enabledCheck
                        text: Style.t("Attiva (sincronizza da sola)", "Active (syncs by itself)")
                        checked: wiz.s.enabled !== false
                        onCheckedChanged: if (checked !== (wiz.s.enabled !== false)) wiz.set("enabled", checked)
                    }
                    MenuSection { width: parent.width; text: Style.t("Riepilogo", "Summary") }
                    Rectangle {
                        width: parent.width
                        height: sumCol.implicitHeight + 2 * Style.margin
                        radius: 12 * Style.dp
                        color: Style.panel
                        border.color: Style.line
                        border.width: 1 * Style.dp
                        Column {
                            id: sumCol
                            x: Style.margin; y: Style.margin
                            width: parent.width - 2 * Style.margin
                            spacing: 4 * Style.dp
                            Text { font.family: Style.fontFamily; width: parent.width; text: wiz.v("name"); color: Style.text; font.pixelSize: Style.fontM; font.bold: true; elide: Text.ElideRight }
                            Text { font.family: Style.fontFamily; width: parent.width; wrapMode: Text.Wrap; color: Style.dim; font.pixelSize: Style.fontS
                                   text: Style.t("Telefono: ", "Phone: ") + wiz.localLabel(wiz.v("local")) + (wiz.s.subdirs ? Style.t(" (con le sottocartelle)", " (with subfolders)") : Style.t(" (senza sottocartelle)", " (without subfolders)")) }
                            Text { font.family: Style.fontFamily; width: parent.width; wrapMode: Text.Wrap; color: Style.dim; font.pixelSize: Style.fontS
                                   text: wiz.connName + ": " + (wiz.v("remotePath") || "/") }
                            Text { font.family: Style.fontFamily; width: parent.width; wrapMode: Text.Wrap; color: Style.dim; font.pixelSize: Style.fontS
                                   text: ({ up: Style.t("Dal telefono al remoto", "Phone to remote"), down: Style.t("Dal remoto al telefono", "Remote to phone"), both: Style.t("Nei due sensi", "Both ways") })[wiz.v("direction")] || "" }
                            Text { font.family: Style.fontFamily; width: parent.width; wrapMode: Text.Wrap; color: Style.dim; font.pixelSize: Style.fontS
                                   text: Style.t("Eliminazioni: ", "Deletions: ") + (({ none: Style.t("non si ripetono", "not repeated"), up: Style.t("dal telefono al remoto", "phone to remote"), down: Style.t("dal remoto al telefono", "remote to phone"), both: Style.t("in entrambi i sensi", "both ways") })[wiz.v("deletions")] || "") }
                        }
                    }
                }
            }
        }

        // ── in basso: indietro / avanti / salva ────────────────────────────
        SteelPanel {
            id: bottomBar
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: -border.width; rightMargin: -border.width }
            anchors.bottomMargin: Qt.inputMethod.visible ? Style.keyboardHeight(wiz.height, wiz.width) : -border.width
            height: Style.bar + 10 * Style.dp
            led: ""
            SteelPill {
                anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                text: wiz.page === 0 ? Style.t("Annulla", "Cancel") : Style.t("Indietro", "Back")
                onClicked: wiz.back()
            }
            SteelPill {
                anchors { right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                role: "accept"
                enabled: wiz.pageOk
                text: wiz.page === 4 ? Style.t("Salva", "Save") : Style.t("Avanti", "Next")
                onClicked: wiz.page === 4 ? wiz.save() : wiz.next()
            }
        }
    }
}
