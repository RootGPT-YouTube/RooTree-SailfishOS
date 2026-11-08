import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// La procedura per aggiungere (o modificare) una connessione remota, a
// pagine come in Solid Explorer ma nello stile di RooTundra: Tipo → Server
// → Accesso → Nome. I cloud con accesso dal browser saltano il server. Il
// profilo si salva in Remotes; la password resta in memoria per la sessione.
Popup {
    id: wiz
    signal saved(string id)

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
    property var p: ({})              // il profilo in costruzione
    property var info: ({})           // il tipo scelto (Remotes.typeInfo)
    property bool editing: false
    property int page: 0
    property string secret
    property bool showSecret: false
    property var keys: []
    onAboutToShow: keys = Fs.sshKeys()
    readonly property var pages: [Style.t("Tipo", "Type"), Style.t("Server", "Server"), Style.t("Accesso", "Sign-in"), Style.t("Nome", "Name")]
    readonly property bool hasServer: info.needsServer === true

    function set(key, value) { var c = Object.assign({}, p); c[key] = value; p = c }
    function v(key) { return p[key] === undefined ? "" : p[key] }
    // il percorso della chiave con «~/» espanso (si può scrivere a mano)
    function keyFile() { var k = v("keyPath"); return k.indexOf("~/") === 0 ? Fs.home + k.substring(1) : k }

    function openNew() {
        editing = false
        p = ({})
        info = ({})
        secret = ""
        showSecret = false
        page = 0
        open()
    }
    function openFor(profile) {
        editing = true
        p = Object.assign({}, profile)
        info = Remotes.typeInfo(profile.type)
        secret = ""
        showSecret = false
        page = hasServer ? 1 : 2
        open()
    }
    function chooseType(t) {
        info = Remotes.typeInfo(t)
        var c = { type: t, port: info.port, auth: info.auths[0], secure: true }
        if (t === "sftp") c.keyPath = keys.length > 0 ? keys[0] : ""
        p = c
        page = hasServer ? 1 : 2
    }

    // quando si può andare avanti
    readonly property bool pageOk: {
        if (page === 0) return info.type !== undefined
        if (page === 1) {
            if (info.type === "s3") return v("bucket") !== "" || v("host") !== ""
            if (info.type === "smb") return v("host") !== "" && v("share") !== ""
            return v("host") !== "" && Number(v("port")) > 0
        }
        if (page === 2) {
            var a = v("auth")
            if (a === "anonymous" || a === "guest" || a === "oauth") return true
            if (a === "key") return v("user") !== "" && v("keyPath") !== ""
            return v("user") !== ""
        }
        return v("name") !== ""
    }
    function next() {
        if (page === 2 && v("name") === "")
            set("name", defaultName())
        page = page + 1
    }
    function back() {
        if (page === 0 || (editing && page === (hasServer ? 1 : 2))) { close(); return }
        page = (page === 2 && !hasServer) ? 0 : page - 1
    }
    function defaultName() {
        if (!hasServer) return info.label + (v("user") ? " · " + v("user") : "")
        if (info.type === "s3" && v("bucket")) return v("bucket")
        return v("host") || info.label
    }
    function save() {
        var c = Object.assign({}, p)
        if (c.keyPath) c.keyPath = keyFile()
        if (secret !== "") c.secret = secret
        var id = Remotes.save(c)
        close()
        saved(id)
    }

    // ── componenti della procedura ─────────────────────────────────────────
    component Label: Text {
        font.family: Style.fontFamily
        width: parent ? parent.width : 0
        color: Style.dim
        font.pixelSize: Style.fontS
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
        onTextEdited: if (key) wiz.set(key, text)
        background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: tf.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
    }
    // un riquadro di un tipo: disco d'acciaio con il simbolo e il nome sotto
    component Tile: Item {
        id: tile
        property var t
        readonly property bool chosen: wiz.info.type === t.type
        width: (typesCol.width - 2 * 8 * Style.dp) / 3
        height: 104 * Style.dp
        Rectangle {
            anchors.fill: parent
            radius: 12 * Style.dp
            color: tap.pressed ? Style.pressed : (tile.chosen ? Style.accentSoft : Style.panel)
            border.color: tile.chosen ? Style.accent : Style.line
            border.width: (tile.chosen ? 2 : 1) * Style.dp
        }
        TypeBadge {
            id: disc
            anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 10 * Style.dp }
            icon: tile.t.icon
            lit: tile.chosen
        }
        Text {
            font.family: Style.fontFamily
            anchors { left: parent.left; right: parent.right; top: disc.bottom; topMargin: 6 * Style.dp; leftMargin: 4 * Style.dp; rightMargin: 4 * Style.dp }
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            text: tile.t.label
            color: Style.text
            font.pixelSize: 12.5 * Style.dp
        }
        MouseArea { id: tap; anchors.fill: parent; onClicked: wiz.chooseType(tile.t.type) }
    }

    FilePicker {
        id: keyPicker
        title: Style.t("Scegli la chiave privata", "Choose the private key")
        markKeys: true
        onPicked: function (path) { wiz.set("keyPath", path) }
    }

    contentItem: Item {
        // ── intestazione: titolo e passi ───────────────────────────────────
        SteelPanel {
            id: top
            anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: -border.width; rightMargin: -border.width; topMargin: -border.width }
            height: Style.bar + steps.height + 10 * Style.dp
            ledWidth: width * (wiz.page + 1) / 4
            led: "bottom"
            IconButton { id: closeBtn; anchors { left: parent.left; top: parent.top; topMargin: 2 * Style.dp } icon: "close"; onClicked: wiz.close() }
            Text {
                font.family: Style.fontFamily
                anchors { left: closeBtn.right; right: parent.right; verticalCenter: closeBtn.verticalCenter; rightMargin: Style.margin }
                text: wiz.editing ? Style.t("Modifica connessione", "Edit connection") : Style.t("Nuova connessione", "New connection")
                color: Style.text
                font.pixelSize: Style.fontL
                font.bold: true
                elide: Text.ElideRight
            }
            // i quattro passi: fatto, attuale, da fare (Server spento per i cloud)
            Row {
                id: steps
                anchors { left: parent.left; right: parent.right; top: closeBtn.bottom; leftMargin: Style.margin; rightMargin: Style.margin }
                height: 34 * Style.dp
                Repeater {
                    model: wiz.pages
                    delegate: Item {
                        width: steps.width / 4
                        height: steps.height
                        readonly property bool skipped: index === 1 && wiz.info.type !== undefined && !wiz.hasServer
                        readonly property bool done: index < wiz.page && !skipped
                        readonly property bool current: index === wiz.page
                        Rectangle {
                            id: dot
                            anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                            width: 20 * Style.dp; height: width; radius: width / 2
                            color: parent.current ? Style.accent : (parent.done ? Style.accentSoft : "transparent")
                            border.color: parent.skipped ? Style.line : Style.accent
                            border.width: 1.5 * Style.dp
                            opacity: parent.skipped ? 0.4 : 1
                            Text { anchors.centerIn: parent; font.family: Style.fontFamily; text: index + 1; color: Style.text; font.pixelSize: 11 * Style.dp; font.bold: true }
                        }
                        Text {
                            font.family: Style.fontFamily
                            anchors { left: dot.right; leftMargin: 5 * Style.dp; right: parent.right; verticalCenter: parent.verticalCenter }
                            text: modelData
                            color: parent.current ? Style.text : Style.dim
                            opacity: parent.skipped ? 0.4 : 1
                            font.pixelSize: Style.fontS
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
            anchors.bottomMargin: Qt.inputMethod.visible ? Math.max(0, Qt.inputMethod.keyboardRectangle.height - bottomBar.height) : 0
            contentHeight: pagesCol.implicitHeight + 2 * Style.margin
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: pagesCol
                x: Style.margin
                y: Style.margin
                width: flick.width - 2 * Style.margin
                spacing: 10 * Style.dp

                // 1 · Tipo
                Column {
                    id: typesCol
                    visible: wiz.page === 0
                    width: parent.width
                    spacing: 8 * Style.dp
                    Label { text: Style.t("Che cosa vuoi collegare?", "What do you want to connect?"); color: Style.text; font.pixelSize: Style.fontM }
                    MenuSection { width: parent.width; text: Style.t("Server", "Servers") }
                    Flow {
                        width: parent.width
                        spacing: 8 * Style.dp
                        Repeater { model: Remotes.types.filter(function (t) { return t.group === "server" }); delegate: Tile { t: modelData } }
                    }
                    MenuSection { width: parent.width; text: "Cloud" }
                    Flow {
                        width: parent.width
                        spacing: 8 * Style.dp
                        Repeater { model: Remotes.types.filter(function (t) { return t.group === "cloud" }); delegate: Tile { t: modelData } }
                    }
                }

                // 2 · Server
                Column {
                    visible: wiz.page === 1
                    width: parent.width
                    spacing: 8 * Style.dp
                    Label { text: wiz.info.label || ""; color: Style.text; font.pixelSize: Style.fontM; font.bold: true }
                    Label {
                        text: wiz.info.type === "s3" ? Style.t("Endpoint (vuoto per Amazon S3)", "Endpoint (empty for Amazon S3)")
                                                     : Style.t("Indirizzo del server", "Server address")
                    }
                    Field { key: "host"; placeholderText: wiz.info.type === "s3" ? "s3.eu-central-1.amazonaws.com" : Style.t("es. 192.168.1.10 o cloud.esempio.it", "e.g. 192.168.1.10 or cloud.example.com"); inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase }
                    Row {
                        width: parent.width
                        spacing: 10 * Style.dp
                        visible: wiz.info.type !== "s3"
                        Column {
                            width: 110 * Style.dp
                            spacing: 4 * Style.dp
                            Label { text: Style.t("Porta", "Port") }
                            Field {
                                text: wiz.v("port") === "" ? "" : String(wiz.v("port"))
                                inputMethodHints: Qt.ImhDigitsOnly
                                validator: IntValidator { bottom: 1; top: 65535 }
                                onTextEdited: wiz.set("port", Number(text))
                            }
                        }
                        Column {
                            width: parent.width - 120 * Style.dp
                            spacing: 4 * Style.dp
                            anchors.bottom: parent.bottom
                            CheckRow {
                                visible: wiz.info.type === "ftp" || wiz.info.type === "webdav" || wiz.info.type === "nextcloud"
                                text: wiz.info.type === "ftp" ? Style.t("Cifrata (FTPS)", "Encrypted (FTPS)") : "HTTPS"
                                checked: wiz.p.secure !== false
                                onCheckedChanged: {
                                    if (checked === (wiz.p.secure !== false)) return
                                    wiz.set("secure", checked)
                                    // la porta segue, se era quella predefinita
                                    if (wiz.info.type !== "ftp") {
                                        if (checked && wiz.p.port === 80) wiz.set("port", 443)
                                        if (!checked && wiz.p.port === 443) wiz.set("port", 80)
                                    }
                                }
                            }
                        }
                    }
                    // il percorso, che cambia nome e senso col tipo
                    Label {
                        visible: wiz.info.type !== "nextcloud" && wiz.info.type !== "s3"
                        text: wiz.info.type === "smb" ? Style.t("Condivisione", "Share")
                            : wiz.info.type === "webdav" ? Style.t("Percorso sul server (es. /dav)", "Path on the server (e.g. /dav)")
                            : Style.t("Cartella iniziale (facoltativa)", "Start folder (optional)")
                    }
                    Field {
                        visible: wiz.info.type !== "nextcloud" && wiz.info.type !== "s3"
                        key: wiz.info.type === "smb" ? "share" : "path"
                        placeholderText: wiz.info.type === "smb" ? Style.t("es. Documenti", "e.g. Documents") : "/"
                    }
                    Label { visible: wiz.info.type === "smb"; text: Style.t("Dominio o gruppo di lavoro (facoltativo)", "Domain or workgroup (optional)") }
                    Field { visible: wiz.info.type === "smb"; key: "domain"; placeholderText: "WORKGROUP" }
                    Label { visible: wiz.info.type === "s3"; text: "Bucket" }
                    Field { visible: wiz.info.type === "s3"; key: "bucket"; placeholderText: Style.t("vuoto: tutti i bucket", "empty: all buckets") }
                    Label { visible: wiz.info.type === "s3"; text: Style.t("Regione", "Region") }
                    Field { visible: wiz.info.type === "s3"; key: "region"; placeholderText: "eu-central-1" }
                    Label {
                        visible: wiz.info.type === "nextcloud"
                        text: Style.t("Il percorso dei file (/remote.php/dav/files/utente) RooTree lo ricava da solo.",
                                      "RooTree works out the files path (/remote.php/dav/files/user) by itself.")
                    }
                }

                // 3 · Accesso
                Column {
                    visible: wiz.page === 2
                    width: parent.width
                    spacing: 8 * Style.dp
                    Label { text: wiz.info.label || ""; color: Style.text; font.pixelSize: Style.fontM; font.bold: true }
                    // il metodo, se ce n'è più d'uno
                    Flow {
                        visible: (wiz.info.auths || []).length > 1
                        width: parent.width
                        spacing: 6 * Style.dp
                        Repeater {
                            model: wiz.info.auths || []
                            delegate: SteelPill {
                                text: ({ password: Style.t("Password", "Password"), key: Style.t("Chiave SSH", "SSH key"),
                                         anonymous: Style.t("Anonimo", "Anonymous"), guest: Style.t("Ospite", "Guest") })[modelData]
                                role: wiz.v("auth") === modelData ? "accept" : ""
                                onClicked: wiz.set("auth", modelData)
                            }
                        }
                    }
                    // accesso dal browser (OAuth)
                    Column {
                        visible: wiz.v("auth") === "oauth"
                        width: parent.width
                        spacing: 8 * Style.dp
                        Label {
                            color: Style.text
                            text: Style.t("Al primo collegamento si aprirà il browser per accedere a " + (wiz.info.label || "") + " e autorizzare RooTree. La password non passa da RooTree.",
                                          "On first connection the browser will open to sign in to " + (wiz.info.label || "") + " and authorise RooTree. Your password never goes through RooTree.")
                        }
                        Label {
                            visible: wiz.info.type === "gdrive"
                            text: Style.t("Facoltativo: un tuo «Client ID» di Google Cloud (gratuito). Senza, Google mostra l'avviso «app non verificata».",
                                          "Optional: your own Google Cloud “Client ID” (free). Without it, Google shows the “unverified app” warning.")
                        }
                        Field { visible: wiz.info.type === "gdrive"; key: "clientId"; placeholderText: "…apps.googleusercontent.com" }
                    }
                    // utente e password / chiave
                    Column {
                        visible: wiz.v("auth") === "password" || wiz.v("auth") === "key"
                        width: parent.width
                        spacing: 8 * Style.dp
                        Label {
                            text: wiz.info.type === "s3" ? "Access key ID" : wiz.info.type === "mega" ? "Email" : Style.t("Utente", "User")
                        }
                        Field { key: "user"; inputMethodHints: wiz.info.type === "mega" ? Qt.ImhEmailCharactersOnly : (Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText) }
                        Label {
                            visible: wiz.v("auth") === "key"
                            text: Style.t("Chiave privata", "Private key")
                        }
                        // il percorso si può scrivere, scegliere sfogliando, o
                        // toccare fra le chiavi trovate in ~/.ssh (richiesta
                        // dell'utente: «so di avere una chiave ma non dove»)
                        Item {
                            visible: wiz.v("auth") === "key"
                            width: parent.width
                            height: keyField.height
                            Field {
                                id: keyField
                                width: parent.width - browseKey.width - 6 * Style.dp
                                key: "keyPath"
                                placeholderText: "~/.ssh/id_ed25519"
                            }
                            SteelPill {
                                id: browseKey
                                anchors { right: parent.right; verticalCenter: keyField.verticalCenter }
                                text: Style.t("Sfoglia…", "Browse…")
                                onClicked: {
                                    var k = wiz.keyFile()
                                    keyPicker.openAt(k && Fs.exists(Fs.parentOf(k)) ? Fs.parentOf(k) : Fs.home + "/.ssh")
                                }
                            }
                        }
                        Label {
                            visible: wiz.v("auth") === "key" && wiz.v("keyPath") !== "" && !Fs.exists(wiz.keyFile())
                            text: Style.t("Questo file non c'è.", "This file does not exist.")
                            color: Style.danger
                        }
                        Label {
                            visible: wiz.v("auth") === "key" && wiz.v("keyPath") !== "" && Fs.exists(wiz.keyFile()) && !Fs.isPrivateKey(wiz.keyFile())
                            text: wiz.v("keyPath").slice(-4) === ".pub"
                                  ? Style.t("Questa è la chiave pubblica: serve la privata (lo stesso nome senza .pub).", "This is the public key: the private one is needed (same name without .pub).")
                                  : Style.t("Non sembra una chiave privata.", "This does not look like a private key.")
                            color: Style.danger
                        }
                        Column {
                            visible: wiz.v("auth") === "key" && wiz.keys.length > 0
                            width: parent.width
                            spacing: 6 * Style.dp
                            Label { text: Style.t("Chiavi trovate in ~/.ssh:", "Keys found in ~/.ssh:") }
                            Flow {
                                width: parent.width
                                spacing: 6 * Style.dp
                                Repeater {
                                    model: wiz.keys
                                    delegate: SteelPill {
                                        text: modelData.split("/").pop()
                                        role: wiz.v("keyPath") === modelData ? "accept" : ""
                                        onClicked: wiz.set("keyPath", modelData)
                                    }
                                }
                            }
                        }
                        Label {
                            visible: wiz.v("auth") === "key" && wiz.keys.length === 0
                            text: Style.t("In ~/.ssh non ho trovato chiavi private: usa «Sfoglia…» per cercarla altrove.", "No private keys found in ~/.ssh: use “Browse…” to look elsewhere.")
                        }
                        Label {
                            text: wiz.v("auth") === "key" ? Style.t("Frase della chiave (se ne ha una)", "Key passphrase (if any)")
                                : wiz.info.type === "s3" ? "Secret access key"
                                : wiz.info.type === "nextcloud" ? Style.t("Password (meglio una «password per app» di Nextcloud)", "Password (better a Nextcloud “app password”)")
                                : Style.t("Password", "Password")
                        }
                        Item {
                            width: parent.width
                            height: secretField.height
                            Field {
                                id: secretField
                                width: parent.width - eye.width - 4 * Style.dp
                                text: wiz.secret
                                echoMode: wiz.showSecret ? TextInput.Normal : TextInput.Password
                                inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
                                placeholderText: wiz.editing && Remotes.hasSecret(wiz.v("id")) ? Style.t("(invariata)", "(unchanged)") : ""
                                onTextEdited: wiz.secret = text
                            }
                            IconButton { id: eye; anchors { right: parent.right; verticalCenter: secretField.verticalCenter } icon: "eye"; checked: wiz.showSecret; onClicked: wiz.showSecret = !wiz.showSecret }
                        }
                        Label {
                            text: Style.t("La password per ora resta solo finché RooTree è aperta; vuota, verrà chiesta al collegamento.",
                                          "For now the password is kept only while RooTree is open; if empty, it is asked when connecting.")
                        }
                    }
                    Label {
                        visible: wiz.v("auth") === "anonymous" || wiz.v("auth") === "guest"
                        color: Style.text
                        text: wiz.v("auth") === "anonymous" ? Style.t("Accesso anonimo: nessun utente né password.", "Anonymous access: no user or password.")
                                                           : Style.t("Accesso come ospite: nessun utente né password.", "Guest access: no user or password.")
                    }
                }

                // 4 · Nome e riepilogo
                Column {
                    visible: wiz.page === 3
                    width: parent.width
                    spacing: 8 * Style.dp
                    Label { text: Style.t("Come la vuoi chiamare?", "What do you want to call it?"); color: Style.text; font.pixelSize: Style.fontM }
                    Field { key: "name" }
                    MenuSection { width: parent.width; text: Style.t("Riepilogo", "Summary") }
                    Rectangle {
                        width: parent.width
                        height: summary.implicitHeight + 2 * Style.margin
                        radius: 12 * Style.dp
                        color: Style.panel
                        border.color: Style.line
                        border.width: 1 * Style.dp
                        Row {
                            id: summaryRow
                            x: Style.margin; y: Style.margin
                            width: parent.width - 2 * Style.margin
                            spacing: Style.margin
                            TypeBadge {
                                id: sumDisc
                                width: 46 * Style.dp
                                icon: wiz.info.icon || ""
                                lit: true
                            }
                            Column {
                                id: summary
                                width: parent.width - sumDisc.width - Style.margin
                                Text { font.family: Style.fontFamily; width: parent.width; text: wiz.v("name"); color: Style.text; font.pixelSize: Style.fontM; font.bold: true; elide: Text.ElideRight }
                                Text { font.family: Style.fontFamily; width: parent.width; text: wiz.info.label || ""; color: Style.dim; font.pixelSize: Style.fontS }
                                Text { font.family: Style.fontFamily; width: parent.width; text: wiz.page === 3 ? Remotes.describe(wiz.p) : ""; color: Style.dim; font.pixelSize: Style.fontS; elide: Text.ElideMiddle }
                                Text {
                                    font.family: Style.fontFamily
                                    width: parent.width
                                    text: ({ password: Style.t("Con password", "With password"), key: Style.t("Con chiave SSH", "With SSH key"),
                                             anonymous: Style.t("Anonimo", "Anonymous"), guest: Style.t("Ospite", "Guest"),
                                             oauth: Style.t("Accesso dal browser", "Browser sign-in") })[wiz.v("auth")] || ""
                                    color: Style.dim
                                    font.pixelSize: Style.fontS
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── in basso: indietro / avanti / salva ────────────────────────────
        SteelPanel {
            id: bottomBar
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: -border.width; rightMargin: -border.width; bottomMargin: -border.width }
            height: Style.bar + 10 * Style.dp
            led: ""
            SteelPill {
                anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                text: wiz.page === 0 || (wiz.editing && wiz.page === (wiz.hasServer ? 1 : 2)) ? Style.t("Annulla", "Cancel") : Style.t("Indietro", "Back")
                onClicked: wiz.back()
            }
            SteelPill {
                visible: wiz.page > 0
                anchors { right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                role: "accept"
                enabled: wiz.pageOk
                text: wiz.page === 3 ? Style.t("Salva", "Save") : Style.t("Avanti", "Next")
                onClicked: wiz.page === 3 ? wiz.save() : wiz.next()
            }
        }
    }
}
