import QtQuick
import RooTree
import "."

// La scheda d'avanzamento (richiesta dell'utente dell'08/11: «di primo
// acchito credevo che il trasferimento non fosse partito»): compare appena
// parte un'operazione e resta finché ce n'è una aperta. Cosa fa, il file
// corrente, una barra col LED (a onda mentre si prepara), percentuale,
// velocità e tempo rimasto, Annulla; un tocco apre l'elenco delle operazioni.
Item {
    id: card
    signal openJobs()

    // la prima operazione aperta (le dimensioni per Proprietà non contano)
    // le sincronizzazioni solo quando copiano o eliminano davvero (non mentre confrontano)
    readonly property var open: Jobs.jobs.filter(function (j) { return (j.kind !== 3 && j.kind < 7 && j.state < 5) || (j.kind === 9 && j.state === 2 && j.totalItems > 0) })
    readonly property var job: open.length > 0 ? open[0] : null
    readonly property bool preparing: job !== null && (job.state === 0 || job.state === 1 || (job.totalBytes === 0 && job.totalItems === 0))
    readonly property real fraction: !job ? 0 : (job.totalBytes > 0 ? job.doneBytes / job.totalBytes
                                                 : (job.totalItems > 0 ? job.doneItems / job.totalItems : 0))

    visible: opacity > 0
    opacity: job !== null ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 180 } }
    height: col.implicitHeight + 2 * 10 * Style.dp

    // velocità: media mobile sugli ultimi secondi, per lavoro
    property int sampleId: -1
    property real lastBytes: 0
    property double lastTime: 0
    property real speed: 0
    onJobChanged: {
        if (!job) { sampleId = -1; speed = 0; return }
        var now = Date.now()
        if (job.id !== sampleId) {
            sampleId = job.id; lastBytes = job.doneBytes; lastTime = now; speed = 0
            return
        }
        var dt = (now - lastTime) / 1000
        if (dt >= 0.5) {
            var inst = (job.doneBytes - lastBytes) / dt
            speed = speed > 0 ? speed * 0.7 + inst * 0.3 : inst
            lastBytes = job.doneBytes
            lastTime = now
        }
    }
    function timeText(s) {
        if (s < 1) return Style.t("meno di 1 s", "under 1 s")
        s = Math.round(s)
        if (s < 60) return s + " s"
        var m = Math.floor(s / 60)
        if (m < 60) return m + ":" + ("0" + (s % 60)).slice(-2)
        return Math.floor(m / 60) + " h " + (m % 60) + " min"
    }
    function what(j) {
        if (!j) return ""
        var n = j.sources
        var w = n === 1 ? "«" + j.firstSource.split("/").pop() + "»" : Style.count(n, Style.t("elemento", "item"), Style.t("elementi", "items"))
        var d = "«" + (j.dest.split("/").pop() || "/") + "»"
        switch (j.kind) {
        case 0: return Style.t("Copia di " + w + " in " + d, "Copying " + w + " to " + d)
        case 1: return Style.t("Spostamento di " + w + " in " + d, "Moving " + w + " to " + d)
        case 2: return Style.t("Eliminazione di " + w, "Deleting " + w)
        case 6: return Style.t("Nel cestino: " + w, "To trash: " + w)
        case 4: return Style.t("Estrazione di " + w + " in " + d, "Extracting " + w + " to " + d)
        case 5: return Style.t("Creazione di " + d, "Creating " + d)
        case 9: return Style.t("Sincronizzazione di «" + (j.firstSource.split("/").pop() || "/") + "» con " + d, "Syncing “" + (j.firstSource.split("/").pop() || "/") + "” with " + d)
        }
        return ""
    }

    SteelPanel {
        anchors.fill: parent
        overlay: true
        radius: 14 * Style.dp
        led: ""
    }
    MouseArea { anchors.fill: parent; onClicked: card.openJobs() }

    Column {
        id: col
        anchors { left: parent.left; right: cancel.left; top: parent.top; leftMargin: Style.margin; topMargin: 10 * Style.dp; rightMargin: 4 * Style.dp }
        spacing: 5 * Style.dp
        Text {
            font.family: Style.fontFamily
            width: parent.width
            text: card.what(card.job) + (card.open.length > 1 ? Style.t("  · +" + (card.open.length - 1) + " in coda", "  · +" + (card.open.length - 1) + " queued") : "")
            color: Style.text
            font.pixelSize: Style.fontM
            font.bold: true
            elide: Text.ElideMiddle
        }
        // la barra: LED blu che avanza, o un'onda mentre ci si prepara
        Rectangle {
            id: track
            width: parent.width
            height: 7 * Style.dp
            radius: height / 2
            color: Style.sailfish ? Style.alpha(Style.sfHighlight, 0.2) : "#20262d"
            border.color: Style.line
            border.width: 1
            clip: true
            Rectangle {
                visible: !card.preparing
                height: parent.height
                radius: parent.radius
                width: Math.max(height, parent.width * card.fraction)
                color: Style.accent
                Behavior on width { NumberAnimation { duration: 140 } }
                // l'alone del LED
                Rectangle { anchors.fill: parent; anchors.margins: -2 * Style.dp; radius: height; color: "transparent"; border.color: Style.alpha(Style.accent, 0.35); border.width: 2 * Style.dp; visible: !Style.sailfish }
            }
            Rectangle {
                id: wave
                visible: card.preparing
                height: parent.height
                radius: parent.radius
                width: parent.width * 0.3
                color: Style.accent
                opacity: 0.85
                SequentialAnimation on x {
                    running: wave.visible
                    loops: Animation.Infinite
                    NumberAnimation { from: -wave.width; to: track.width; duration: 1100; easing.type: Easing.InOutQuad }
                }
            }
        }
        Text {
            font.family: Style.fontFamily
            width: parent.width
            color: Style.dim
            font.pixelSize: Style.fontS
            elide: Text.ElideMiddle
            text: {
                var j = card.job
                if (!j) return ""
                if (j.state === 3 || j.state === 4) return Style.t("Aspetta una tua risposta", "Waiting for your answer")
                if (card.preparing) return Style.t("Preparazione… (conto file e dimensioni)", "Preparing… (counting files and sizes)")
                var parts = [Math.floor(card.fraction * 100) + " %"]
                if (j.totalBytes > 0) parts.push(Style.size(j.doneBytes) + " / " + Style.size(j.totalBytes))
                if (card.speed > 1024) {
                    parts.push(Style.size(card.speed) + "/s")
                    if (j.totalBytes > j.doneBytes) parts.push(Style.t(card.timeText((j.totalBytes - j.doneBytes) / card.speed) + " al termine", card.timeText((j.totalBytes - j.doneBytes) / card.speed) + " left"))
                }
                return parts.join(" · ")
            }
        }
        Text {
            font.family: Style.fontFamily
            width: parent.width
            visible: text !== ""
            color: Style.dim
            font.pixelSize: 12 * Style.dp
            elide: Text.ElideMiddle
            text: card.job && !card.preparing && card.job.current ? card.job.current.split("/").pop() : ""
        }
    }
    IconButton {
        id: cancel
        anchors { right: parent.right; verticalCenter: parent.verticalCenter; rightMargin: 2 * Style.dp }
        icon: "close"
        onClicked: if (card.job) Jobs.cancel(card.job.id)
    }
}
