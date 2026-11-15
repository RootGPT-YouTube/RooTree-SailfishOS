import QtQuick
import RooTree
import "."

// Il cestino di una memoria (locale) o di una connessione: i file eliminati,
// da ripristinare o da togliere per sempre. `cid` = id della connessione, o
// la cartella locale dove si è (comincia con «/»: il cestino della sua memoria).
Sheet {
    id: sheet
    property string cid
    property string where
    // Dropbox: si ripristina soltanto (il definitivo l'API non lo concede)
    property bool restoreOnly: false
    // le impostazioni dell'app (trashDays)
    property var store
    property var items: []
    property bool busy: false
    property string error
    signal restored()
    title: Style.t("Cestino", "Trash") + (where !== "" ? " · " + where : "")

    function openFor(p) {
        start(p.id, p.name || "", p.type === "dropbox")
    }
    function openLocal(path, name) {
        start(path, name, false)
    }
    function start(id, name, onlyRestore) {
        cid = id
        where = name
        restoreOnly = onlyRestore
        items = []
        error = ""
        open()
        reload()
    }
    readonly property var dayChoices: [7, 30, 90, 0]
    function reload() {
        busy = true
        Remotes.trash(cid, "list", "")
    }
    function run(action, item) {
        busy = true
        error = ""
        Remotes.trash(cid, action, item)
    }
    function sizeText(n) {
        if (n < 1024) return n + " B"
        var u = ["KB", "MB", "GB", "TB"], i = -1
        do { n /= 1024; i++ } while (n >= 1024 && i < u.length - 1)
        return n.toFixed(n < 10 ? 1 : 0) + " " + u[i]
    }

    Connections {
        target: Remotes
        function onTrashDone(id, action, err, json) {
            if (id !== sheet.cid) return
            sheet.busy = false
            if (err !== "") {
                var c = err.indexOf(":")
                sheet.error = err.indexOf("NOTCONNECTED:") === 0 ? Style.t("Apri prima la connessione.", "Open the connection first.")
                            : /^[A-Z]+:/.test(err) ? err.substring(c + 1) : err
                return
            }
            if (action === "list") {
                sheet.items = JSON.parse(json)
                return
            }
            if (action === "restore") sheet.restored()
            sheet.reload()
        }
    }

    Text {
        font.family: Style.fontFamily
        visible: text !== ""
        x: Style.margin
        width: parent.width - 2 * Style.margin
        wrapMode: Text.Wrap
        bottomPadding: 6 * Style.dp
        text: sheet.error !== "" ? sheet.error
            : sheet.busy && sheet.items.length === 0 ? Style.t("Lettura del cestino…", "Reading the trash…")
            : sheet.items.length === 0 ? Style.t("Il cestino è vuoto.", "The trash is empty.")
            : ""
        color: sheet.error !== "" ? Style.danger : Style.dim
        font.pixelSize: Style.fontM
    }
    MenuRow {
        // sempre in vista (da vuoto non si capiva che c'è, 12/11), grigio se non c'è niente
        enabled: !sheet.busy && sheet.items.length > 0
        visible: !sheet.restoreOnly
        icon: "delete"
        text: Style.t("Svuota il cestino", "Empty the trash")
        onClicked: { confirm.item = ""; confirm.label = ""; confirm.open() }
    }
    // quanto restano le voci: solo nei cestini di RooTree (memorie e server
    // senza cestino proprio); Nextcloud, Mega e Dropbox hanno le loro regole
    MenuRow {
        readonly property bool own: sheet.cid.charAt(0) === "/" || ["nextcloud", "mega", "dropbox", "kdrive"].indexOf(Remotes.get(sheet.cid).type || "") < 0
        visible: own
        icon: "refresh"
        text: Style.t("Svuotamento automatico", "Automatic emptying")
        detail: sheet.store.trashDays > 0 ? Style.t("dopo " + sheet.store.trashDays + " giorni", "after " + sheet.store.trashDays + " days") : Style.t("mai", "never")
        onClicked: {
            var i = sheet.dayChoices.indexOf(sheet.store.trashDays)
            sheet.store.trashDays = sheet.dayChoices[(i + 1) % sheet.dayChoices.length]
        }
    }
    Text {
        font.family: Style.fontFamily
        visible: sheet.restoreOnly
        x: Style.margin
        width: parent.width - 2 * Style.margin
        wrapMode: Text.Wrap
        bottomPadding: 6 * Style.dp
        text: Style.t("Dropbox toglie da solo i file eliminati dopo 30 giorni; prima si possono solo ripristinare (o togliere dal sito).",
                      "Dropbox removes deleted files by itself after 30 days; until then they can only be restored (or removed on the website).")
        color: Style.dim
        font.pixelSize: Style.fontS
    }
    Repeater {
        model: sheet.items
        delegate: Item {
            width: parent.width
            height: Style.row + 12 * Style.dp
            Icon {
                id: kind
                anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                name: modelData.dir ? "folder" : "copy"
            }
            Column {
                anchors { left: kind.right; leftMargin: Style.margin; right: restoreBtn.left; verticalCenter: parent.verticalCenter }
                Text {
                    font.family: Style.fontFamily
                    width: parent.width
                    text: modelData.name
                    color: Style.text
                    font.pixelSize: Style.fontM
                    elide: Text.ElideMiddle
                }
                Text {
                    font.family: Style.fontFamily
                    width: parent.width
                    // da dove viene, quando, quanto; «root» = cestino di root
                    text: [modelData.root ? "root" : "",
                           modelData.deleted > 0 ? Qt.formatDateTime(new Date(modelData.deleted * 1000), "dd/MM/yyyy HH:mm") : "",
                           modelData.size > 0 ? sheet.sizeText(modelData.size) : "",
                           modelData.original || ""].filter(function (x) { return x !== "" }).join(" · ")
                    color: Style.dim
                    font.pixelSize: Style.fontS
                    elide: Text.ElideRight
                }
            }
            IconButton {
                id: restoreBtn
                anchors { right: deleteBtn.left; verticalCenter: parent.verticalCenter }
                icon: "back"
                enabled: !sheet.busy
                onClicked: sheet.run("restore", modelData.id)
            }
            IconButton {
                id: deleteBtn
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                icon: "delete"
                visible: !sheet.restoreOnly
                width: visible ? implicitWidth : 0
                enabled: !sheet.busy
                onClicked: { confirm.item = modelData.id; confirm.label = modelData.name; confirm.open() }
            }
        }
    }

    AppDialog {
        id: confirm
        property string item
        property string label
        title: item !== "" ? Style.t("Eliminare per sempre?", "Delete forever?") : Style.t("Svuotare il cestino?", "Empty the trash?")
        message: item !== ""
            ? Style.t("«" + label + "» non si potrà più recuperare.", "“" + label + "” can no longer be recovered.")
            : Style.t("Tutti i file del cestino non si potranno più recuperare.", "All files in the trash can no longer be recovered.")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Elimina", "Delete"), role: "danger" }]
        onChosen: function (role) {
            if (role === "danger") sheet.run(item !== "" ? "delete" : "empty", item)
        }
    }
}
