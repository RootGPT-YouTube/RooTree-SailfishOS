import QtQuick
import RooTree
import "."

// L'elenco delle sincronizzazioni (menù ⋮ → Opzioni per Remoto →
// Sincronizzazioni): per ognuna l'ultimo giro; un tocco apre le sue azioni.
Sheet {
    id: sheet
    title: Style.t("Sincronizzazioni", "Syncs")
    signal newRequested()
    signal runRequested(string id)
    signal runAllRequested()
    signal editRequested(var sync)
    signal deleteRequested(var sync)

    function arrow(s) { return s.direction === "down" ? " ← " : s.direction === "both" ? " ⇄ " : " → " }
    function when(ms) {
        if (!ms) return Style.t("mai sincronizzata", "never synced")
        var d = new Date(ms), now = new Date()
        var hm = ("0" + d.getHours()).slice(-2) + ":" + ("0" + d.getMinutes()).slice(-2)
        return d.toDateString() === now.toDateString() ? Style.t("oggi alle ", "today at ") + hm : Style.date(ms / 1000)
    }

    Text {
        font.family: Style.fontFamily
        visible: Syncs.list.length === 0
        x: Style.margin
        width: parent.width - 2 * Style.margin
        wrapMode: Text.Wrap
        text: Style.t("Nessuna sincronizzazione. Una sincronizzazione tiene una cartella del telefono allineata con una cartella di un NAS, di un server o di un cloud.",
                      "No syncs. A sync keeps a phone folder aligned with a folder on a NAS, a server or a cloud.")
        color: Style.dim
        font.pixelSize: Style.fontS
        bottomPadding: 8 * Style.dp
    }
    Repeater {
        model: Syncs.list
        delegate: Rectangle {
            id: row
            width: parent ? parent.width : 0
            height: rowCol.implicitHeight + 16 * Style.dp
            color: rowTap.pressed ? Style.pressed : "transparent"
            readonly property var conn: Remotes.get(modelData.remote)
            readonly property bool busy: Jobs.jobs.some(function (j) { return j.kind === 9 && j.state < 5 && j.info && j.info.syncId === modelData.id })
            TypeBadge {
                id: badge
                anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                width: 40 * Style.dp
                icon: Remotes.typeInfo(row.conn.type || "").icon || "cloud"
                lit: row.busy
                opacity: modelData.enabled === false ? 0.45 : 1
            }
            Column {
                id: rowCol
                anchors { left: badge.right; leftMargin: Style.margin; right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                spacing: 1 * Style.dp
                Text { font.family: Style.fontFamily; width: parent.width; text: modelData.name; color: Style.text; font.pixelSize: Style.fontM; font.bold: true; elide: Text.ElideRight }
                Text {
                    font.family: Style.fontFamily; width: parent.width; elide: Text.ElideMiddle; color: Style.dim; font.pixelSize: Style.fontS
                    text: modelData.local.split("/").pop() + sheet.arrow(modelData) + (row.conn.name || Style.t("connessione eliminata", "deleted connection")) + " " + (modelData.remotePath || "/")
                }
                Text {
                    font.family: Style.fontFamily; width: parent.width; wrapMode: Text.Wrap; font.pixelSize: Style.fontS
                    color: modelData.lastError ? Style.danger : Style.dim
                    text: row.busy ? Style.t("Sincronizzazione in corso…", "Syncing…")
                          : modelData.enabled === false ? Style.t("Disattivata (solo a mano)", "Disabled (manual only)") + " · " + sheet.when(modelData.lastRun)
                          : sheet.when(modelData.lastRun) + (modelData.lastResult ? " · " + modelData.lastResult : "")
                }
            }
            Rectangle { anchors { left: rowCol.left; right: parent.right; bottom: parent.bottom } height: 1; color: Style.line; opacity: 0.6 }
            MouseArea { id: rowTap; anchors.fill: parent; onClicked: { syncMenu.sync = modelData; syncMenu.open() } }
        }
    }
    MenuRow { icon: "refresh"; visible: Syncs.list.length > 0; text: Style.t("Sincronizza tutto ora", "Sync all now"); onClicked: { sheet.close(); sheet.runAllRequested() } }
    MenuRow { icon: "add"; text: Style.t("Nuova sincronizzazione…", "New sync…"); onClicked: { sheet.close(); sheet.newRequested() } }

    Sheet {
        id: syncMenu
        property var sync: ({})
        title: sync.name || ""
        MenuRow { icon: "refresh"; text: Style.t("Sincronizza ora", "Sync now"); onClicked: { syncMenu.close(); sheet.close(); sheet.runRequested(syncMenu.sync.id) } }
        MenuRow { icon: "openwith"; text: Style.t("Modifica", "Edit"); onClicked: { syncMenu.close(); sheet.close(); sheet.editRequested(syncMenu.sync) } }
        MenuRow {
            icon: "check"
            text: Style.t("Attiva (sincronizza da sola)", "Active (syncs by itself)")
            checkable: true
            checked: syncMenu.sync.enabled !== false
            onClicked: {
                var c = Object.assign({}, syncMenu.sync)
                c.enabled = c.enabled === false
                Syncs.save(c)
                syncMenu.sync = c
            }
        }
        MenuRow { icon: "delete"; text: Style.t("Elimina", "Delete"); onClicked: { syncMenu.close(); sheet.close(); sheet.deleteRequested(syncMenu.sync) } }
    }
}
