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
    property var configHosts: []
    onAboutToShow: { keys = Fs.sshKeys(); configHosts = Fs.sshConfigHosts() }
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
    // un server di ~/.ssh/config: connessione SFTP già compilata, si va al
    // riepilogo (da lì si torna indietro a correggere)
    function fromConfig(h) {
        info = Remotes.typeInfo("sftp")
        p = { type: "sftp", name: h.alias, host: h.host, port: h.port, user: h.user,
              auth: h.identityFile ? "key" : "password", keyPath: h.identityFile || "", secure: true }
        page = 3
    }
    function chooseType(t) {
        info = Remotes.typeInfo(t)
        var c = { type: t, port: info.port, auth: info.auths[0], secure: true }
        // con una chiave sola la si propone; con più chiavi si sceglie (la
        // prima in ordine alfabetico poteva essere quella sbagliata)
        if (t === "sftp") c.keyPath = keys.length === 1 ? keys[0] : ""
        p = c
        page = hasServer ? 1 : 2
    }

    // quando si può andare avanti
    readonly property bool pageOk: {
        if (page === 0) return info.type !== undefined
        if (page === 1) {
            // S3: Amazon con tutti i bucket ha endpoint e bucket vuoti, e va bene
            if (info.type === "s3") return true
            if (info.type === "smb") return v("host") !== ""
            return v("host") !== "" && Number(v("port")) > 0
        }
        if (page === 2) {
            var a = v("auth")
            if (a === "oauth") return v("clientId").trim() !== ""
            if (a === "anonymous" || a === "guest") return true
            // kDrive: l'indirizzo (o il numero) del drive è obbligatorio: col solo
            // permesso «drive» il token non può cercarlo (utente 15/11)
            if (a === "token") return /^\s*\d+\s*$/.test(v("drive")) || /\/drive\/\d+/.test(v("drive"))
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
        // un cloud si chiama come il servizio: l'account non va in vista
        if (!hasServer) return info.label
        if (info.type === "s3") return v("bucket") || (v("host") ? v("host").replace(/^https?:\/\//, "") : "Amazon S3")
        if (info.type === "nextcloud") return "Nextcloud"
        return v("host") || info.label
    }
    function hideKeyboard() {
        Qt.inputMethod.commit()
        flick.forceActiveFocus()
        Qt.inputMethod.hide()
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
        // Invio: la tastiera si chiude e ricompaiono i pulsanti
        onAccepted: wiz.hideKeyboard()
        background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: tf.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
    }
    // un riquadro di un tipo: disco d'acciaio con il simbolo e il nome sotto
    component Tile: Item {
        id: tile
        property var t
        readonly property bool chosen: wiz.info.type === t.type
        // un cloud alla volta: quelli non ancora pronti si vedono ma non si scelgono
        readonly property bool soon: t.soon === true
        opacity: soon ? 0.45 : 1
        width: (typesCol.width - 2 * 8 * Style.dp) / 3
        height: 112 * Style.dp
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
            text: tile.t.label + (tile.soon ? "\n" + Style.t("in arrivo", "coming") : "")
            color: Style.text
            font.pixelSize: 12.5 * Style.dp
        }
        MouseArea { id: tap; anchors.fill: parent; enabled: !tile.soon; onClicked: wiz.chooseType(tile.t.type) }
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
            contentHeight: pagesCol.implicitHeight + 2 * Style.margin
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            // un tocco fuori dalle caselle chiude la tastiera (sta sotto a tutto:
            // caselle, pulsanti e riquadri ricevono i loro tocchi come prima)
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

                // 1 · Tipo
                Column {
                    id: typesCol
                    visible: wiz.page === 0
                    width: parent.width
                    spacing: 8 * Style.dp
                    Label { text: Style.t("Che cosa vuoi collegare?", "What do you want to connect?"); color: Style.text; font.pixelSize: Style.fontM }
                    // i server già definiti per ssh, pronti da importare
                    MenuSection { visible: wiz.configHosts.length > 0; width: parent.width; text: Style.t("Da ~/.ssh/config", "From ~/.ssh/config") }
                    Repeater {
                        model: wiz.configHosts
                        delegate: Rectangle {
                            width: typesCol.width
                            height: Style.row + 4 * Style.dp
                            radius: 12 * Style.dp
                            color: hostTap.pressed ? Style.pressed : Style.panel
                            border.color: Style.line
                            border.width: 1 * Style.dp
                            TypeBadge { id: hb; anchors { left: parent.left; leftMargin: 8 * Style.dp; verticalCenter: parent.verticalCenter } width: 40 * Style.dp; icon: "ssh" }
                            Column {
                                anchors { left: hb.right; leftMargin: Style.margin; right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                                Text {
                                    font.family: Style.fontFamily; width: parent.width; elide: Text.ElideRight
                                    text: modelData.alias + (modelData.aliases.length > 1 ? "  (" + modelData.aliases.slice(1).join(", ") + ")" : "")
                                    color: Style.text; font.pixelSize: Style.fontM; font.bold: true
                                }
                                Text {
                                    font.family: Style.fontFamily; width: parent.width; elide: Text.ElideMiddle
                                    text: (modelData.user ? modelData.user + "@" : "") + modelData.host + (modelData.port !== 22 ? ":" + modelData.port : "")
                                          + (modelData.identityFile ? " · " + Style.t("chiave ", "key ") + modelData.identityFile.split("/").pop() : "")
                                    color: Style.dim; font.pixelSize: Style.fontS
                                }
                            }
                            MouseArea { id: hostTap; anchors.fill: parent; onClicked: wiz.fromConfig(modelData) }
                        }
                    }
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
                    Label {
                        visible: wiz.info.type === "ftp"
                        text: wiz.p.secure !== false
                            ? (Number(wiz.v("port")) === 990
                               ? Style.t("Porta 990: FTPS «implicito», cifrato fin dal primo byte.", "Port 990: “implicit” FTPS, encrypted from the first byte.")
                               : Style.t("FTPS esplicito: il collegamento si cifra prima di mandare la password. Se il server non offre FTPS, togli «Cifrata»; per l'FTPS «implicito» usa la porta 990.",
                                         "Explicit FTPS: the connection is encrypted before the password is sent. If the server does not offer FTPS, untick “Encrypted”; for “implicit” FTPS use port 990."))
                            : Style.t("⚠ Senza cifratura la password e i file viaggiano leggibili da chiunque sia sulla rete: va bene solo in casa.",
                                      "⚠ Without encryption the password and the files travel readable by anyone on the network: fine only at home.")
                    }
                    // il percorso, che cambia nome e senso col tipo
                    Label {
                        visible: wiz.info.type !== "nextcloud" && wiz.info.type !== "s3"
                        text: wiz.info.type === "smb" ? Style.t("Condivisione (facoltativa)", "Share (optional)")
                            : wiz.info.type === "webdav" ? Style.t("Percorso sul server (es. /dav)", "Path on the server (e.g. /dav)")
                            : Style.t("Cartella iniziale (facoltativa)", "Start folder (optional)")
                    }
                    Field {
                        visible: wiz.info.type !== "nextcloud" && wiz.info.type !== "s3"
                        key: wiz.info.type === "smb" ? "share" : "path"
                        placeholderText: wiz.info.type === "smb" ? Style.t("vuota: le scegli dall'elenco del server", "empty: pick from the server's list") : "/"
                    }
                    Label {
                        visible: wiz.info.type === "smb"
                        text: Style.t("Le cartelle condivise di Windows, Samba e dei NAS (SMB 2 e 3, cifratura compresa). Senza condivisione si parte dall'elenco di quelle del server.",
                                      "Shared folders of Windows, Samba and NAS devices (SMB 2 and 3, encryption included). Without a share you start from the server's list.")
                    }
                    Label { visible: wiz.info.type === "smb"; text: Style.t("Dominio o gruppo di lavoro (facoltativo)", "Domain or workgroup (optional)") }
                    Field { visible: wiz.info.type === "smb"; key: "domain"; placeholderText: "WORKGROUP" }
                    Label { visible: wiz.info.type === "s3"; text: "Bucket" }
                    Field { visible: wiz.info.type === "s3"; key: "bucket"; placeholderText: Style.t("vuoto: tutti i bucket", "empty: all buckets") }
                    Label { visible: wiz.info.type === "s3"; text: Style.t("Regione", "Region") }
                    Field { visible: wiz.info.type === "s3"; key: "region"; placeholderText: Style.t("vuota: la cerca da sola", "empty: found by itself") }
                    Label {
                        visible: wiz.info.type === "s3"
                        text: Style.t("Amazon S3: endpoint vuoto. MinIO, Garage o lo S3 di un NAS: http://indirizzo:porta (es. http://192.168.1.10:9000). Backblaze B2, Wasabi, Cloudflare R2: l'endpoint indicato dal servizio. Senza bucket si parte dall'elenco dei bucket.",
                                      "Amazon S3: empty endpoint. MinIO, Garage or a NAS's S3: http://address:port (e.g. http://192.168.1.10:9000). Backblaze B2, Wasabi, Cloudflare R2: the endpoint the service gives. Without a bucket you start from the list of buckets.")
                    }
                    Label {
                        visible: wiz.info.type === "nextcloud"
                        text: Style.t("Il percorso dei file (/remote.php/dav/files/utente) RooTree lo ricava da solo.",
                                      "RooTree works out the files path (/remote.php/dav/files/user) by itself.")
                    }
                    // la procedura per Nextcloud, solo alla creazione
                    Rectangle {
                        visible: !wiz.editing && (wiz.info.type === "webdav" || wiz.info.type === "nextcloud")
                        width: parent.width
                        height: guide.height + 20 * Style.dp
                        radius: 8 * Style.dp
                        color: Style.panel
                        border.color: Style.line
                        border.width: 1 * Style.dp
                        Column {
                            id: guide
                            x: 10 * Style.dp
                            y: 10 * Style.dp
                            width: parent.width - 20 * Style.dp
                            spacing: 8 * Style.dp
                            Label { color: Style.text; font.bold: true; text: Style.t("Collegare un account Nextcloud", "Connecting a Nextcloud account") }
                            Label {
                                color: Style.danger
                                text: Style.t("Se sull'account è attiva la verifica in due passaggi (TOTP), la password normale viene rifiutata: qui il codice non si può inserire. Serve una password per app. Troppi tentativi sbagliati possono far bloccare il tuo indirizzo dal server.",
                                              "If two-factor authentication (TOTP) is on for the account, the normal password is refused: the code cannot be entered here. You need an app password. Too many failed attempts may get your address blocked by the server.")
                            }
                            Label {
                                color: Style.text
                                text: Style.t("1. Dal browser entra in https://indirizzo-server con password e codice.\n2. Vai in Impostazioni personali › Sicurezza › Dispositivi e sessioni e crea una password per app.\n3. Qui inserisci indirizzo-server, HTTPS e, nella pagina successiva, utente e password per app.",
                                              "1. In the browser, sign in to https://server-address with password and code.\n2. Go to Personal settings › Security › Devices & sessions and create an app password.\n3. Here enter server-address, HTTPS and, on the next page, user and app password.")
                            }
                            Label {
                                visible: wiz.info.type === "webdav"
                                text: Style.t("Percorso sul server (tocca per copiare, poi sostituisci «utente» col tuo nome utente):",
                                              "Path on the server (tap to copy, then replace “user” with your user name):")
                            }
                            Rectangle {
                                id: pathCopy
                                visible: wiz.info.type === "webdav"
                                readonly property string davPath: Style.t("/remote.php/dav/files/utente/", "/remote.php/dav/files/user/")
                                property bool copied: false
                                width: parent.width
                                height: 40 * Style.dp
                                radius: 6 * Style.dp
                                color: copyTap.pressed ? Style.pressed : Style.bg
                                border.color: pathCopy.copied ? Style.ok : Style.line
                                border.width: 1.5 * Style.dp
                                Text {
                                    anchors { left: parent.left; leftMargin: 8 * Style.dp; right: copyIcon.left; rightMargin: 6 * Style.dp; verticalCenter: parent.verticalCenter }
                                    font.family: Style.fontFamily
                                    font.pixelSize: Style.fontS
                                    color: pathCopy.copied ? Style.ok : Style.text
                                    elide: Text.ElideMiddle
                                    text: pathCopy.copied ? Style.t("Copiato", "Copied") : pathCopy.davPath
                                }
                                Icon { id: copyIcon; anchors { right: parent.right; rightMargin: 8 * Style.dp; verticalCenter: parent.verticalCenter } name: "copy"; size: 20 * Style.dp }
                                // il testo passa dagli appunti di sistema
                                TextEdit { id: copyBuffer; visible: false; text: pathCopy.davPath }
                                Timer { id: copiedTimer; interval: 2000; onTriggered: pathCopy.copied = false }
                                MouseArea {
                                    id: copyTap
                                    anchors.fill: parent
                                    onClicked: {
                                        copyBuffer.selectAll()
                                        copyBuffer.copy()
                                        copyBuffer.deselect()
                                        pathCopy.copied = true
                                        copiedTimer.restart()
                                    }
                                }
                            }
                        }
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
                            text: Style.t("Al collegamento si aprirà il browser per accedere a " + (wiz.info.label || "") + " e autorizzare RooTree. La password non passa da RooTree.",
                                          "On connecting the browser will open to sign in to " + (wiz.info.label || "") + " and authorise RooTree. Your password never goes through RooTree.")
                        }
                        Label {
                            text: wiz.info.type === "dropbox" ? "App key"
                                : Style.t("ID dell'applicazione (client)", "Application (client) ID")
                        }
                        Field { key: "clientId"; placeholderText: wiz.info.type === "dropbox" ? "abc123xyz…" : "" }
                        // come avere la chiave: gratis, una volta sola (RooTree non usa servizi a carico dell'autore)
                        Rectangle {
                            width: parent.width
                            height: howto.implicitHeight + 2 * Style.margin
                            radius: 10 * Style.dp
                            color: Style.panel
                            border.color: Style.line
                            border.width: 1 * Style.dp
                            Text {
                                id: howto
                                x: Style.margin; y: Style.margin
                                width: parent.width - 2 * Style.margin
                                font.family: Style.fontFamily
                                color: Style.dim
                                font.pixelSize: Style.fontS
                                wrapMode: Text.Wrap
                                // i siti da visitare sono link toccabili (richiesta dell'utente,
                                // 08/11); gli indirizzi da incollare restano testo
                                textFormat: Text.StyledText
                                linkColor: Style.accent
                                onLinkActivated: function (link) { Opener.openUrl(link) }
                                function linkify(t) {
                                    return t.replace(/\n/g, "<br>")
                                        .replace("dropbox.com/developers/apps", '<a href="https://www.dropbox.com/developers/apps">dropbox.com/developers/apps</a>')
                                }
                                text: linkify(wiz.info.type === "dropbox"
                                    ? Style.t("Come avere la App key (gratis, una volta sola):\n1. Su dropbox.com/developers/apps: «Create app» → «Scoped access» → «Full Dropbox» (o «App folder»), un nome a piacere.\n2. Scheda «Permissions» («Autorizzazioni»): spunta SOLO queste caselle: account_info.write, account_info.read, files.metadata.write, files.metadata.read, files.content.write, files.content.read (quelle .read possono essere già spuntate e grigie). Lascia VUOTE tutte le altre, compresa files.permanent_delete e tutte quelle del team: con un permesso di team Dropbox risponde «Accedi con un account di lavoro». Poi conferma con «Submit» in fondo.\n3. Scheda «Settings» («Impostazioni»), «OAuth 2» → «Redirect URIs»: aggiungi http://127.0.0.1:53682/ (proprio così). «Allow public clients» deve restare su «Allow».\n4. Copia qui la «App key» (la «App secret» non serve).",
                                              "How to get the App key (free, once):\n1. On dropbox.com/developers/apps: “Create app” → “Scoped access” → “Full Dropbox” (or “App folder”), any name.\n2. “Permissions” tab: tick ONLY these boxes: account_info.write, account_info.read, files.metadata.write, files.metadata.read, files.content.write, files.content.read (the .read ones may already be ticked and greyed out). Leave ALL the others EMPTY, including files.permanent_delete and every team permission: with a team permission Dropbox answers “Sign in with a work account”. Then confirm with “Submit” at the bottom.\n3. “Settings” tab, “OAuth 2” → “Redirect URIs”: add http://127.0.0.1:53682/ (exactly). “Allow public clients” must stay on “Allow”.\n4. Copy the “App key” here (the “App secret” is not needed).")
                                    : Style.t("Serve un ID dell'app tuo (gratuito): le istruzioni arriveranno con questo servizio.", "Your own (free) app ID is needed: instructions will come with this service."))
                            }
                        }
                        CheckRow {
                            text: Style.t("Ricorda l'accesso nella cassaforte", "Remember the sign-in in the vault")
                            checked: wiz.p.remember !== false
                            onCheckedChanged: if (checked !== (wiz.p.remember !== false)) wiz.set("remember", checked)
                        }
                    }
                    // utente e password / chiave / token (kDrive)
                    Column {
                        visible: wiz.v("auth") === "password" || wiz.v("auth") === "key" || wiz.v("auth") === "token"
                        width: parent.width
                        spacing: 8 * Style.dp
                        // kDrive: come avere il token (gratis anche con kSuite gratuita, provato dall'utente il 15/11)
                        Rectangle {
                            visible: wiz.v("auth") === "token"
                            width: parent.width
                            height: tokenHowto.implicitHeight + 2 * Style.margin
                            radius: 10 * Style.dp
                            color: Style.panel
                            border.color: Style.line
                            border.width: 1 * Style.dp
                            Text {
                                id: tokenHowto
                                x: Style.margin; y: Style.margin
                                width: parent.width - 2 * Style.margin
                                font.family: Style.fontFamily
                                color: Style.dim
                                font.pixelSize: Style.fontS
                                wrapMode: Text.Wrap
                                textFormat: Text.StyledText
                                linkColor: Style.accent
                                onLinkActivated: function (link) { Opener.openUrl(link) }
                                readonly property string link: '<a href="https://manager.infomaniak.com/v3/ng/accounts/token/list">manager.infomaniak.com → Token API</a>'
                                text: Style.t("Come avere il token (gratis, una volta sola):<br>1. Apri " + link + " e accedi.<br>2. «Crea un token»: un nome a piacere, permesso (scope) <b>drive</b>, durata a scelta.<br>3. Copialo qui: Infomaniak lo mostra una volta sola.<br>Il token apre il tuo kDrive: tienilo nella cassaforte e non darlo a nessuno.",
                                              "How to get the token (free, once):<br>1. Open " + link + " and sign in.<br>2. “Create a token”: any name, scope <b>drive</b>, any duration.<br>3. Paste it here: Infomaniak shows it only once.<br>The token opens your kDrive: keep it in the vault and give it to no one.")
                            }
                        }
                        Label {
                            visible: wiz.v("auth") !== "token"
                            text: wiz.info.type === "s3" ? "Access key ID" : wiz.info.type === "mega" ? "Email" : Style.t("Utente", "User")
                        }
                        Field { visible: wiz.v("auth") !== "token"; key: "user"; inputMethodHints: wiz.info.type === "mega" ? Qt.ImhEmailCharactersOnly : (Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText) }
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
                            Label {
                                text: wiz.keys.length === 1 ? Style.t("Chiave trovata in ~/.ssh:", "Key found in ~/.ssh:")
                                                            : Style.t("Chiavi trovate in ~/.ssh: tocca quella giusta", "Keys found in ~/.ssh: tap the right one")
                            }
                            Repeater {
                                model: wiz.keys
                                delegate: Rectangle {
                                    readonly property var ki: Fs.sshKeyInfo(modelData)
                                    readonly property bool chosen: wiz.keyFile() === modelData
                                    width: parent.width
                                    height: keyCol.implicitHeight + 14 * Style.dp
                                    radius: 10 * Style.dp
                                    color: keyTap.pressed ? Style.pressed : (chosen ? Style.accentSoft : Style.panel)
                                    border.color: chosen ? Style.accent : Style.line
                                    border.width: (chosen ? 2 : 1) * Style.dp
                                    Icon { id: keyIcon; anchors { left: parent.left; leftMargin: 10 * Style.dp; verticalCenter: parent.verticalCenter } name: "key"; size: 18 * Style.dp }
                                    Column {
                                        id: keyCol
                                        anchors { left: keyIcon.right; leftMargin: 10 * Style.dp; right: parent.right; rightMargin: 10 * Style.dp; verticalCenter: parent.verticalCenter }
                                        Text { font.family: Style.fontFamily; width: parent.width; text: modelData.split("/").pop(); color: Style.text; font.pixelSize: Style.fontM; font.bold: true; elide: Text.ElideMiddle }
                                        Text {
                                            font.family: Style.fontFamily; width: parent.width; elide: Text.ElideRight
                                            visible: text !== ""
                                            text: [parent.parent.ki.type, parent.parent.ki.comment].filter(function (x) { return x }).join(" · ")
                                            color: Style.dim; font.pixelSize: Style.fontS
                                        }
                                        Text {
                                            font.family: Style.fontFamily; width: parent.width; elide: Text.ElideRight
                                            visible: parent.parent.ki.hosts.length > 0
                                            text: Style.t("usata per: ", "used for: ") + parent.parent.ki.hosts.join(", ")
                                            color: Style.accent; font.pixelSize: Style.fontS
                                        }
                                    }
                                    MouseArea { id: keyTap; anchors.fill: parent; onClicked: wiz.set("keyPath", modelData) }
                                }
                            }
                        }
                        Label {
                            visible: wiz.v("auth") === "key" && wiz.keys.length === 0
                            text: Style.t("In ~/.ssh non ho trovato chiavi private: usa «Sfoglia…» per cercarla altrove.", "No private keys found in ~/.ssh: use “Browse…” to look elsewhere.")
                        }
                        Label {
                            text: wiz.v("auth") === "key" ? Style.t("Frase della chiave (se ne ha una)", "Key passphrase (if any)")
                                : wiz.v("auth") === "token" ? Style.t("Token dell'API", "API token")
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
                        Label { visible: wiz.v("auth") === "token"; text: Style.t("Indirizzo del tuo kDrive", "Your kDrive address") }
                        Field {
                            visible: wiz.v("auth") === "token"
                            key: "drive"
                            inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                            placeholderText: "https://ksuite.infomaniak.com/…/drive/…"
                        }
                        Label {
                            visible: wiz.v("auth") === "token"
                            textFormat: Text.StyledText
                            text: Style.t("Apri kDrive nel browser, copia l'indirizzo dalla barra e incollalo qui. Somiglia a:<br>https://ksuite.infomaniak.com/9999999/kdrive/app/drive/<b><font color=\"#4fa3ff\">1234567</font></b>/files/5<br>RooTree ne prende il numero dopo «/drive/» (qui <b>1234567</b>): puoi anche scrivere solo quello.",
                                          "Open kDrive in a browser, copy the address from the bar and paste it here. It looks like:<br>https://ksuite.infomaniak.com/9999999/kdrive/app/drive/<b><font color=\"#4fa3ff\">1234567</font></b>/files/5<br>RooTree takes the number after “/drive/” (here <b>1234567</b>): you can also type just that.")
                        }
                        CheckRow {
                            text: Style.t("Ricorda nella cassaforte", "Remember in the vault")
                            checked: wiz.p.remember !== false
                            onCheckedChanged: if (checked !== (wiz.p.remember !== false)) wiz.set("remember", checked)
                        }
                        Label {
                            text: wiz.v("auth") === "token"
                                  ? Style.t("La cassaforte è cifrata con una password principale tua. Vuoto qui, il token verrà chiesto al collegamento.",
                                            "The vault is encrypted with your own master password. Left empty here, the token is asked when connecting.")
                                  : Style.t("La cassaforte è cifrata con una password principale tua. Vuota qui, la password verrà chiesta al collegamento.",
                                            "The vault is encrypted with your own master password. Left empty here, the password is asked when connecting.")
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
                    // anche qui (chi arriva da ~/.ssh/config salta la pagina Server)
                    Label {
                        visible: wiz.hasServer && wiz.info.type !== "smb" && wiz.info.type !== "s3" && wiz.info.type !== "nextcloud" && wiz.info.type !== "webdav"
                        text: Style.t("Cartella da cui aprire (vuoto: la home sul server)", "Folder to open from (empty: home on the server)")
                    }
                    Field {
                        visible: wiz.hasServer && wiz.info.type !== "smb" && wiz.info.type !== "s3" && wiz.info.type !== "nextcloud" && wiz.info.type !== "webdav"
                        key: "path"
                        placeholderText: "/media/…"
                    }
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
                                             oauth: Style.t("Accesso dal browser", "Browser sign-in"), token: Style.t("Con token dell'API", "With API token") })[wiz.v("auth")] || ""
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
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: -border.width; rightMargin: -border.width }
            // con la tastiera aperta i pulsanti le stanno sopra, sempre a portata
            anchors.bottomMargin: Qt.inputMethod.visible ? Style.keyboardHeight(wiz.height, wiz.width) : -border.width
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
