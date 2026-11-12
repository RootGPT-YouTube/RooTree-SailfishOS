import QtQuick
import RooTree

// Collaudo del cestino di root: un file in /etc, modalità root, nel cestino,
// elenco (voce «root»), ripristino; poi di nuovo nel cestino e definitivo.
Item {
    property string dir: Qt.application.arguments[1]
    property string f: "/etc/rootree-prova-cestino.conf"
    property string step
    function log() { console.log("ROOTC " + Array.prototype.join.call(arguments, " ")) }
    Component.onCompleted: {
        log("hint senza root:", Fs.trashHint([f]))
        Fs.rootStart(Opener.readText(dir + "/pw").text.replace(/\n$/, ""))
    }
    Connections {
        target: Fs
        function onRootFinished(code, message) {
            log("root:", code, message, "attiva=" + Fs.rootActive)
            if (!Fs.rootActive) { Qt.quit(); return }
            log("hint con root:", Fs.trashHint([f]))
            step = "trash"
            Jobs.trash([f])
        }
    }
    Connections {
        target: Jobs
        function onFinished(id, kind, state, error) {
            log("lavoro kind=" + kind, "state=" + state, "errore='" + error + "'", "esiste=" + Fs.exists(f))
            if (step === "trash") { step = "list"; Remotes.trash("/etc", "list", "") }
            else if (step === "trash2") { step = "list2"; Remotes.trash("/etc", "list", "") }
        }
        function onJobsChanged() { var q = Jobs.question; if (q.id) { log("DOMANDA", q.error); Jobs.resolve(q.id, 3, false) } }
    }
    property string itemId
    Connections {
        target: Remotes
        function onTrashDone(id, action, err, json) {
            if (err !== "") log("ERRORE", action, err)
            if (step === "list" || step === "list2") {
                var mine = JSON.parse(json).filter(function (v) { return v.original === f })
                log(step, JSON.stringify(mine))
                if (mine.length === 0) { finish(); return }
                itemId = mine[0].id
                if (step === "list") { step = "restore"; Remotes.trash("/etc", "restore", itemId) }
                else { step = "delete"; Remotes.trash("/etc", "delete", itemId) }
            } else if (step === "restore") {
                log("ripristinato:", json, "esiste=" + Fs.exists(f))
                step = "trash2"; Jobs.trash([f])
            } else if (step === "delete") {
                log("definitivo: esiste in cestino=" + Fs.exists(itemId))
                finish()
            }
        }
    }
    function finish() { Fs.rootStop(); log("FINE"); Qt.quit() }
}
