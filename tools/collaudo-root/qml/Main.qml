import QtQuick
import RooTree

// Prova fuori schermo del collegamento QML ↔ Fs/Jobs ↔ helper root (le stesse
// chiamate dei pulsanti), con un file di prova in /etc. Non va nel pacchetto.
Item {
    // la cartella di lavoro sul telefono (contiene pw, vedi LEGGIMI.md)
    property string dir: Qt.application.arguments.length > 1 ? Qt.application.arguments[1] : "/home/debugging/collaudo-root"
    property int job: 0
    property int ticks: 0
    FolderModel { id: folder; path: "/home" }
    function log() { console.log(Array.prototype.join.call(arguments, " ")) }
    Component.onCompleted: {
        log("modalità sviluppatore:", Fs.developerMode, "helper installato:", Fs.rootHelperInstalled, "attiva:", Fs.rootActive)
        log("/root da utente:", Fs.isWritable("/etc"), "scrivibile /etc")
        Fs.rootStart("sbagliata")
    }
    property int round: 0
    Connections {
        target: Fs
        function onRootFinished(code, message) {
            log("rootFinished code=" + code, "msg=" + message, "attiva=" + Fs.rootActive, "starting=" + Fs.rootStarting)
            if (round++ === 0) {
                var pw = Opener.readText(dir + "/pw").text.replace(/\n$/, "")
                Fs.rootStart(pw)
                return
            }
            if (code !== "") { Qt.quit(); return }
            folder.path = "/root"
            log("/root:", folder.count, "voci, errore=" + folder.error)
            log("/etc scrivibile ora:", Fs.isWritable("/etc"))
            var r = Fs.writeText("/etc/rootree-prova.conf", "salvato da QML\n")
            log("writeText errore=" + r.error, "copia=" + r.backup)
            log("setOwner:", "'" + Fs.setOwner("/etc/rootree-prova.conf", "", "video") + "'", "gruppo ora:", Fs.info("/etc/rootree-prova.conf").group)
            job = Jobs.copy(["/etc/rootree-prova.conf"], dir)
            log("lavoro root id=" + job)
            poll.start()
        }
    }
    Timer {
        id: poll; interval: 100; repeat: true
        onTriggered: {
            var st = Jobs.status(job)
            if (ticks++ % 10 === 0) log("tick", JSON.stringify(st), "jobs:", JSON.stringify(Jobs.jobs.map(function (j) { return [j.id, j.state] })))
            if (st.state >= 5) {
                stop()
                log("lavoro finito stato=" + st.state, st.error)
                var i = Fs.info(dir + "/rootree-prova.conf")
                log("copia in home: proprietario=" + i.user, "gruppo=" + i.group)
                Fs.rootStop()
                log("dopo stop attiva=" + Fs.rootActive)
                folder.refresh()
                log("/root dopo stop:", folder.count, "errore=" + folder.error)
                Qt.quit()
            }
        }
    }
}
