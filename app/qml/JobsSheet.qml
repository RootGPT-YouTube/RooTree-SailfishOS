import QtQuick
import RooTree
import "."

// Le operazioni: in corso, in coda e finite, con l'avanzamento e Annulla.
Sheet {
    id: sheet
    title: Style.t("Operazioni", "Operations")

    function describe(j) {
        var n = j.sources
        var what = n === 1 ? "«" + j.firstSource.split("/").pop() + "»" : Style.count(n, Style.t("elemento", "item"), Style.t("elementi", "items"))
        var dest = j.dest.split("/").pop() || "/"
        switch (j.kind) {
        case 0: return Style.t("Copia di " + what + " in «" + dest + "»", "Copying " + what + " to “" + dest + "”")
        case 1: return Style.t("Spostamento di " + what + " in «" + dest + "»", "Moving " + what + " to “" + dest + "”")
        case 2: return Style.t("Eliminazione di " + what, "Deleting " + what)
        case 6: return Style.t("Nel cestino: " + what, "To trash: " + what)
        case 4: return Style.t("Estrazione di " + what + " in «" + dest + "»", "Extracting " + what + " to “" + dest + "”")
        case 5: return Style.t("Creazione di «" + j.dest.split("/").pop() + "»", "Creating “" + j.dest.split("/").pop() + "”")
        }
        return ""
    }
    function stateText(j) {
        switch (j.state) {
        case 0: return Style.t("In coda", "Queued")
        case 1: return Style.t("Calcolo in corso…", "Scanning…")
        case 2: return j.totalBytes > 0 ? Style.size(j.doneBytes) + " / " + Style.size(j.totalBytes)
                                        : j.doneItems + " / " + j.totalItems
        case 3: return Style.t("Aspetta una risposta: il file esiste già", "Waiting: the file already exists")
        case 4: return Style.t("Aspetta una risposta: errore", "Waiting: error")
        case 5: return Style.t("Completata", "Done")
        case 6: return Style.t("Non riuscita: ", "Failed: ") + j.error
        case 7: return Style.t("Annullata", "Cancelled")
        }
        return ""
    }
    function fraction(j) {
        if (j.state === 5) return 1
        if (j.totalBytes > 0) return j.doneBytes / j.totalBytes
        return j.totalItems > 0 ? j.doneItems / j.totalItems : 0
    }

    Text {
        font.family: Style.fontFamily
        visible: rep.count === 0
        x: Style.margin
        width: parent.width - 2 * Style.margin
        height: Style.row
        verticalAlignment: Text.AlignVCenter
        text: Style.t("Nessuna operazione", "No operations")
        color: Style.dim
        font.pixelSize: Style.fontM
    }
    // un modello stabile, aggiornato sul posto: ricreare le righe a ogni
    // lettura dello stato (ogni 150 ms) distruggeva il pulsante fra pressione e
    // rilascio, e Annulla non arrivava (visto sul POCO, 25/10)
    ListModel { id: rows }
    function sync() {
        var list = Jobs.jobs.filter(function (j) { return j.kind !== 3 && j.kind < 7 })
        for (var i = rows.count - 1; i >= 0; --i) {
            var id = rows.get(i).job.id
            if (!list.some(function (j) { return j.id === id }))
                rows.remove(i)
        }
        for (var k = 0; k < list.length; ++k) {
            var at = -1
            for (var r = 0; r < rows.count; ++r)
                if (rows.get(r).job.id === list[k].id) { at = r; break }
            if (at < 0)
                rows.append({ job: list[k] })
            else
                rows.setProperty(at, "job", list[k])
        }
    }
    Connections { target: Jobs; function onJobsChanged() { if (sheet.opened) sheet.sync() } }
    onAboutToShow: sync()

    Repeater {
        id: rep
        model: rows
        delegate: Item {
            readonly property var modelData: job
            width: parent.width
            height: info.implicitHeight + 20 * Style.dp
            Column {
                id: info
                anchors.left: parent.left
                anchors.leftMargin: Style.margin
                anchors.right: action.left
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4 * Style.dp
                Text { font.family: Style.fontFamily; width: parent.width; text: sheet.describe(modelData); color: Style.text; font.pixelSize: Style.fontM; elide: Text.ElideMiddle }
                Rectangle {
                    width: parent.width
                    height: 4 * Style.dp
                    radius: height / 2
                    color: Style.line
                    Rectangle {
                        width: parent.width * sheet.fraction(modelData)
                        height: parent.height
                        radius: parent.radius
                        color: modelData.state === 6 ? Style.danger : (modelData.state === 5 ? Style.ok : (modelData.state === 7 ? Style.dim : Style.accent))
                    }
                }
                Text {
                    font.family: Style.fontFamily
                    width: parent.width
                    text: sheet.stateText(modelData)
                    color: modelData.state === 6 ? Style.danger : Style.dim
                    font.pixelSize: Style.fontS
                    wrapMode: Text.Wrap
                }
                Text {
                    font.family: Style.fontFamily
                    visible: modelData.state === 2 && modelData.current !== ""
                    width: parent.width
                    text: modelData.current.split("/").pop()
                    color: Style.dim
                    font.pixelSize: Style.fontS
                    elide: Text.ElideMiddle
                }
            }
            IconButton {
                id: action
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                icon: "close"
                onClicked: modelData.state < 5 ? Jobs.cancel(modelData.id) : Jobs.forget(modelData.id)
            }
        }
    }
    MenuRow {
        visible: Jobs.jobs.some(function (j) { return j.kind !== 3 && j.kind < 7 && j.state >= 5 })
        icon: "delete"
        text: Style.t("Togli dall'elenco quelle finite", "Clear finished")
        onClicked: Jobs.forgetFinished()
    }
}
