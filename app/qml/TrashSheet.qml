import QtQuick
import RooTree
import "."

// Il cestino di una connessione Nextcloud: i file eliminati, da ripristinare
// o da togliere per sempre.
Sheet {
    id: sheet
    property string cid
    property var items: []
    property bool busy: false
    property string error
    signal restored()
    title: Style.t("Cestino", "Trash")

    function openFor(p) {
        cid = p.id
        items = []
        error = ""
        open()
        reload()
    }
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
                sheet.error = /^[A-Z]+:/.test(err) ? err.substring(c + 1) : err
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
        enabled: !sheet.busy
        visible: sheet.items.length > 0
        icon: "delete"
        text: Style.t("Svuota il cestino", "Empty the trash")
        onClicked: { confirm.item = ""; confirm.label = ""; confirm.open() }
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
                    text: Qt.formatDateTime(new Date(modelData.deleted * 1000), "dd/MM/yyyy HH:mm")
                          + (modelData.size > 0 ? " · " + sheet.sizeText(modelData.size) : "")
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
