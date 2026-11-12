import QtQuick
import RooTree

// Collaudo del cestino senza schermo: per ogni memoria crea, mette nel
// cestino, elenca, ripristina, elimina per sempre.
Item {
    property var bases: Qt.application.arguments.slice(1)
    property int bi: -1
    property string base
    property string step
    property var listed: []
    function log() { console.log("CESTINO " + Array.prototype.join.call(arguments, " ")) }
    Component.onCompleted: { Fs.trashSetDays(30); next() }
    function next() {
        bi++
        if (bi >= bases.length) { log("FINE"); Qt.quit(); return }
        base = bases[bi]
        log("=== memoria", base)
        log("makeDir:", "'" + Fs.makeDir(base, "c") + "'", "makeFile:", "'" + Fs.makeFile(base + "/c", "f.txt") + "'", "'" + Fs.makeFile(base, "g.txt") + "'")
        log("makeFile di nuovo (deve dare errore):", "'" + Fs.makeFile(base, "g.txt") + "'")
        log("hint:", Fs.trashHint([base + "/c", base + "/g.txt"]))
        step = "trash"
        Jobs.trash([base + "/c", base + "/g.txt"])
    }
    Connections {
        target: Jobs
        function onFinished(id, kind, state, error) {
            log("lavoro kind=" + kind, "state=" + state, "errore='" + error + "'")
            log("dopo: c esiste=" + Fs.exists(base + "/c"), "g.txt esiste=" + Fs.exists(base + "/g.txt"))
            if (step === "trash") { step = "list1"; Remotes.trash(base, "list", "") }
        }
        function onJobsChanged() {
            var q = Jobs.question
            if (q.id) { log("DOMANDA", JSON.stringify(q)); Jobs.resolve(q.id, 3, false) }
        }
    }
    function mine(json) {
        return JSON.parse(json).filter(function (v) { return v.original.indexOf(base + "/") === 0 })
    }
    Connections {
        target: Remotes
        function onTrashDone(id, action, err, json) {
            if (err !== "") log("ERRORE", action, err)
            if (step === "list1") {
                listed = mine(json)
                log("nel cestino:", JSON.stringify(listed))
                var c = listed.filter(function (v) { return v.name === "c" })[0]
                step = "restore"
                Remotes.trash(base, "restore", c.id)
            } else if (step === "restore") {
                log("ripristinato in", json, "f.txt c'è=" + Fs.exists(base + "/c/f.txt"))
                var g = listed.filter(function (v) { return v.name === "g.txt" })[0]
                step = "delete"
                Remotes.trash(base, "delete", g.id)
            } else if (step === "delete") {
                step = "list2"
                Remotes.trash(base, "list", "")
            } else if (step === "list2") {
                log("rimasti nel cestino per questa prova:", mine(json).length)
                // pulizia: la cartella ripristinata
                step = "clean"
                Jobs.remove([base + "/c"])
                cleanTimer.start()
            }
        }
    }
    Timer { id: cleanTimer; interval: 1500; onTriggered: { log("pulita: c esiste=" + Fs.exists(base + "/c")); step = ""; next() } }
}
