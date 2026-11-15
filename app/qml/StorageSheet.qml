import QtQuick
import QtQuick.Controls.Basic
import QtCore
import RooTree
import "."

// La pagina della «i» di una memoria: spazio con la torta (per tipo di file o
// per cartella), file system, memoria fisica, stato e, per schede e chiavette,
// il controllo del file system. I dati vengono dal nucleo (Fs.storageInfo,
// Jobs.types, Jobs.fsCheck: storage.rs); i contatori di ext4 da /sys con
// Opener.readText. Prima versione nel solo QML di POCO Agent (15/11/2026),
// motore nella 0.9.3.
Sheet {
    id: sheet
    property var place: ({})
    property var si: ({})        // Fs.storageInfo
    property var phys: []        // memoria fisica: [{ label, value, tone }]
    property var health: []      // stato: [{ label, value, tone }]
    // vista della torta: "types" (per tipo di file) o "folders" (per cartella)
    property string view: "types"
    // per tipo di file: il lavoro, i dati (dal lavoro o dalla cache) e quando
    property int typesJob: 0
    property var typesInfo: null
    property real typesTime: 0
    property string expanded: ""
    // per cartella
    property var cats: []
    property bool scanning: false
    // il controllo della scheda
    property int checkJob: 0
    property var checkStatus: ({})
    property var checkResult: null
    property bool showOutput: false
    // su «/»: il file che contiene la memoria interna cifrata, se c'è (loop)
    property var homeFile: null

    signal openFolder(string path)

    readonly property real used: Math.max(0, (place.total || 0) - (place.free || 0))
    readonly property bool removable: place.kind === 2 || place.kind === 3

    title: {
        switch (place.kind) {
        case 0: return Style.t("Memoria interna", "Internal storage")
        case 1: return Style.t("Memoria Android", "Android storage")
        case 2: return Style.t("Scheda SD", "SD card")
        case 3: return place.label || "USB"
        case 4: return Style.t("File system (/)", "File system (/)")
        }
        return ""
    }

    // l'ultima misura per tipo di ogni memoria: la pagina la mostra subito
    Settings {
        id: store
        category: "storage"
        property string typesCache: "{}"
    }

    // ── formati ─────────────────────────────────────────────────────────────
    function rd(path) {
        var r = Opener.readText(path)
        return (r && r.text !== undefined) ? r.text.trim() : ""
    }
    function pct(part, whole) {
        if (!whole) return ""
        var p = 100 * part / whole
        if (p > 0 && p < 0.1) return "<" + Number(0.1).toLocaleString(Qt.locale(), 'f', 1) + "%"
        return Number(p).toLocaleString(Qt.locale(), 'f', p < 10 ? 1 : 0) + "%"
    }
    function millions(n) {
        if (n >= 1e6) return Number(n / 1e6).toLocaleString(Qt.locale(), 'f', 1) + Style.t(" milioni", " million")
        return Number(n).toLocaleString(Qt.locale(), 'f', 0)
    }
    function ago(secs) {
        var d = Math.max(0, Date.now() / 1000 - secs)
        if (d < 90) return Style.t("adesso", "just now")
        if (d < 5400) return Style.t(Math.round(d / 60) + " min fa", Math.round(d / 60) + " min ago")
        if (d < 129600) return Style.t(Math.round(d / 3600) + " ore fa", Math.round(d / 3600) + " hours ago")
        return Style.t(Math.round(d / 86400) + " giorni fa", Math.round(d / 86400) + " days ago")
    }
    function baseName(p) { return p.slice(p.lastIndexOf("/") + 1) }
    function dirName(p) { var i = p.lastIndexOf("/"); return i > 0 ? p.slice(0, i) : "/" }

    readonly property var sdMakers: ({ "0x000001": "Panasonic", "0x000002": "Toshiba / Kioxia", "0x000003": "SanDisk",
        "0x00001b": "Samsung", "0x00001d": "ADATA", "0x000027": "Phison", "0x000028": "Lexar", "0x000031": "Silicon Power",
        "0x000041": "Kingston", "0x000074": "Transcend", "0x000076": "Patriot", "0x000082": "Sony", "0x00009c": "Angelbird" })
    function ufsLife(v) {
        var n = parseInt(v, 16)
        if (isNaN(n) || n === 0) return Style.t("non dichiarata", "not reported")
        if (n >= 11) return Style.t("oltre la durata prevista", "past rated life")
        return ((n - 1) * 10) + "–" + (n * 10) + "%"
    }
    function ufsEol(v) {
        switch (parseInt(v, 16)) {
        case 1: return Style.t("normale", "normal")
        case 2: return Style.t("attenzione: usato l'80% dei blocchi di riserva", "warning: 80% of reserve blocks used")
        case 3: return Style.t("critica: riserva quasi esaurita", "critical: reserve nearly exhausted")
        }
        return Style.t("non dichiarata", "not reported")
    }

    // ── la catena dei dispositivi (dal nucleo) ──────────────────────────────
    function chainNodes(n, out) {
        if (!n) return out
        out.push(n)
        var b = n.below || []
        if (b.length) chainNodes(b[0], out)
        return out
    }
    readonly property var chain: chainNodes(si.chain, [])
    readonly property var disk: chain.length ? chain[chain.length - 1] : null
    readonly property var cryptNode: { for (var i = 0; i < chain.length; ++i) if (chain[i].crypt) return chain[i]; return null }
    function nodeLabel(n) {
        switch (n.kind) {
        case "dm": return (n.dmName || n.name) + (n.crypt ? " (" + n.crypt + ")" : "")
        case "loop": return n.name + (n.backingFile ? Style.t(" (file ", " (file ") + n.backingFile + ")" : "")
        case "disk": return n.transport ? n.transport.toUpperCase() + " " + n.name : n.name
        }
        return n.name
    }
    function diskType(d) {
        if (!d) return ""
        switch (d.transport) {
        case "ufs": return "UFS" + (removable ? "" : Style.t(" (memoria interna del telefono)", " (phone's internal storage)"))
        case "emmc": return "eMMC"
        case "sd": return d.size > 32e9 ? "SDXC" : (d.size > 2.1e9 ? "SDHC" : "SD")
        case "usb": return "USB"
        case "nvme": return "NVMe"
        }
        return d.transport || ""
    }

    function load() {
        var phy = [], st = []
        var d = disk
        if (d) {
            phy.push({ label: Style.t("Tipo", "Type"), value: diskType(d) })
            var mk = d.manfid ? sdMakers[d.manfid] : ""
            var model = d.model || ((mk ? mk + " " : "") + (d.mmcName || ""))
            if (model) phy.push({ label: Style.t("Modello", "Model"), value: model })
            if (d.firmware) phy.push({ label: Style.t("Firmware", "Firmware"), value: d.firmware })
            var cap = d.chipSize || d.size
            if (cap) phy.push({ label: d.chipSize ? Style.t("Capacità del chip", "Chip capacity") : Style.t("Capacità", "Capacity"), value: Style.size(cap) })
            if (d.date) phy.push({ label: Style.t("Prodotta", "Made"), value: d.date })
            if (d.serial) phy.push({ label: Style.t("Numero di serie", "Serial"), value: d.serial })
            // la UFS dichiara l'usura a decine (0x02 = tra il 10 e il 20% della
            // vita prevista); A è l'area veloce, B il grosso della memoria
            if (d.lifeA || d.lifeB)
                phy.push({ label: Style.t("Usura", "Wear"),
                           value: (d.lifeA === d.lifeB ? ufsLife(d.lifeA) : "A " + ufsLife(d.lifeA) + " · B " + ufsLife(d.lifeB))
                                  + Style.t(" della vita prevista (stima del chip, a scatti di 10%)", " of rated life (chip estimate, in 10% steps)") })
            if (d.eol) phy.push({ label: Style.t("Riserva blocchi", "Reserve blocks"), value: ufsEol(d.eol), tone: parseInt(d.eol, 16) >= 2 ? "danger" : "" })
        }
        if (chain.length > 1)
            phy.push({ label: Style.t("Catena", "Stack"), value: chain.map(nodeLabel).join(" → ") })

        // stato: ext4 tiene il conto degli errori e dei dati scritti
        var dev = si.device || ""
        if (si.fstype === "ext4" && dev) {
            var e = "/sys/fs/ext4/" + dev + "/"
            var errs = parseInt(rd(e + "errors_count"))
            if (!isNaN(errs)) {
                var t1 = parseInt(rd(e + "first_error_time")), t2 = parseInt(rd(e + "last_error_time"))
                var days = t2 ? Math.floor((Date.now() / 1000 - t2) / 86400) : 0
                var txt = Style.t("nessuno", "none"), tone = ""
                if (errs > 0 && t2 && days >= 7) {
                    // errori vecchi: il contatore non si azzera da solo (lo fa solo
                    // un controllo a file system smontato), ma da allora niente
                    txt = Style.t(errs + " in passato (primo " + Style.date(t1) + ", ultimo " + Style.date(t2) + "): nessuno negli ultimi " + days + " giorni",
                                  errs + " in the past (first " + Style.date(t1) + ", last " + Style.date(t2) + "): none in the last " + days + " days")
                    tone = "amber"
                } else if (errs > 0) {
                    txt = String(errs) + (t1 ? Style.t(" (primo ", " (first ") + Style.date(t1) + Style.t(", ultimo ", ", last ") + Style.date(t2) + ")" : "")
                    tone = "danger"
                }
                st.push({ label: Style.t("Errori registrati", "Recorded errors"), value: txt, tone: tone })
            }
            var life = parseInt(rd(e + "lifetime_write_kbytes")), sess = parseInt(rd(e + "session_write_kbytes"))
            if (!isNaN(life)) st.push({ label: Style.t("Scritti in tutto", "Written in total"), value: Style.size(life * 1024) + Style.t(" da quando è stato creato", " since creation") })
            if (si.ioCounted === false)
                st.push({ label: Style.t("Scritti ora", "Written now"), value: Style.t("non contati: il kernel non tiene il conto per questa partizione", "not counted: the kernel keeps no count for this partition") })
            else if (!isNaN(sess))
                st.push({ label: Style.t("Scritti ora", "Written now"), value: Style.size(sess * 1024) + Style.t(" da questa accensione", " since this boot") })
        } else if (si.fstype) {
            var onErr = /errors=([a-z-]+)/.exec(si.options || "")
            st.push({ label: Style.t("Errori registrati", "Recorded errors"),
                      value: Style.t(si.fstype + " non ne tiene il conto", si.fstype + " does not count them")
                             + (onErr ? Style.t(" (a un errore: ", " (on error: ") + onErr[1] + ")" : "") })
        }
        var sb = si.superblock
        if (sb) {
            st.push({ label: Style.t("Stato del file system", "File system state"),
                      value: sb.errorsFlag ? Style.t("con errori: va controllato", "has errors: needs a check") : (sb.clean ? Style.t("pulito", "clean") : Style.t("montato (non ancora chiuso)", "mounted (not closed yet)")),
                      tone: sb.errorsFlag ? "danger" : "" })
            st.push({ label: Style.t("Ultimo controllo", "Last check"), value: sb.lastCheck ? Style.date(sb.lastCheck) : Style.t("mai", "never") })
            st.push({ label: Style.t("Montato", "Mounted"), value: Style.t(sb.mountCount + " volte dall'ultimo controllo", sb.mountCount + " times since the last check") })
        } else if (si.superblockNeedsRoot) {
            st.push({ label: Style.t("Ultimo controllo", "Last check"), value: Style.t("si legge con la modalità root", "readable in root mode") })
        }
        st.push({ label: Style.t("Settori danneggiati", "Bad sectors"),
                  value: Style.t("le memorie flash sostituiscono da sole i blocchi guasti con quelli di riserva, e il sistema non li vede: a dire quanto è consumata è l'usura qui sopra (dove c'è).",
                                 "flash storage swaps failing blocks for reserve ones by itself, invisibly to the system: wear (above, where reported) tells how worn it is.") })
        phys = phy
        health = st
    }

    // ── per tipo di file ────────────────────────────────────────────────────
    readonly property var typeDefs: [
        { key: "images", name: Style.t("Immagini", "Pictures"), color: "#4fa3ff" },
        { key: "video", name: Style.t("Video", "Videos"), color: "#b66dff" },
        { key: "audio", name: Style.t("Musica e audio", "Music and audio"), color: "#ff6fae" },
        { key: "documents", name: Style.t("Documenti", "Documents"), color: "#ffc247" },
        { key: "archives", name: Style.t("Archivi", "Archives"), color: "#ff8a4c" },
        { key: "packages", name: Style.t("Pacchetti (APK, RPM)", "Packages (APK, RPM)"), color: "#2ec4b6" },
        { key: "apps", name: Style.t("App e loro dati", "Apps and their data"), color: "#9ccc3c" },
        { key: "cache", name: Style.t("Cache", "Cache"), color: "#d9a066" },
        { key: "system", name: Style.t("Sistema", "System"), color: "#6f8fb5" },
        { key: "other", name: Style.t("Altri file", "Other files"), color: "#8a96a3" }
    ]
    readonly property bool typesRunning: typesJob !== 0
    function startTypes() {
        stopTypes()
        if (!place.path) return
        typesJob = Jobs.types([place.path])
    }
    function stopTypes() {
        if (typesJob) { Jobs.cancel(typesJob); Jobs.forget(typesJob) }
        typesJob = 0
    }
    function cacheGet() {
        try { return JSON.parse(store.typesCache)[place.path] || null } catch (e) { return null }
    }
    function cachePut(info) {
        var all = {}
        try { all = JSON.parse(store.typesCache) } catch (e) {}
        all[place.path] = { time: Date.now() / 1000, info: info }
        store.typesCache = JSON.stringify(all)
    }

    // ── per cartella ────────────────────────────────────────────────────────
    readonly property var catDefs: [
        { name: Style.t("Immagini", "Pictures"), color: "#4fa3ff", dirs: ["Pictures", "DCIM"] },
        { name: Style.t("Video", "Videos"), color: "#b66dff", dirs: ["Videos", "Movies"] },
        { name: Style.t("Musica", "Music"), color: "#ff6fae", dirs: ["Music", "Audiobooks", "Podcasts"] },
        { name: Style.t("Documenti", "Documents"), color: "#ffc247", dirs: ["Documents"] },
        { name: Style.t("Download", "Downloads"), color: "#5ccf8a", dirs: ["Downloads", "Download"] },
        // senza root i dati delle app (Android/data, Android/obb) della memoria
        // Android non si leggono; sulle schede (exFAT, FAT) sì
        { name: Fs.rootActive ? Style.t("Android", "Android") : Style.t("Android (senza dati app)", "Android (no app data)"), color: "#9ccc3c", dirs: ["android_storage"] },
        { name: (place.kind === 1 && !Fs.rootActive) ? Style.t("Dati app Android (serve root)", "Android app data (needs root)") : Style.t("Dati app Android", "Android app data"), color: "#9ccc3c", dirs: ["Android"] }
    ]
    function startScan() {
        stopScan()
        var out = []
        if (place.kind !== 4 && place.path) {
            catDefs.forEach(function (c) {
                var found = c.dirs.map(function (n) { return Fs.join(place.path, n) }).filter(function (p) { return Fs.exists(p) })
                if (found.length)
                    out.push({ name: c.name, color: c.color, job: Jobs.size(found), bytes: 0, done: false })
            })
        }
        cats = out
        scanning = out.length > 0
    }
    function stopScan() {
        cats.forEach(function (c) { if (c.job) { if (!c.done) Jobs.cancel(c.job); Jobs.forget(c.job) } })
        cats = []
        scanning = false
    }

    Connections {
        target: Jobs
        enabled: sheet.scanning || sheet.typesJob !== 0 || sheet.checkJob !== 0
        function onJobsChanged() {
            if (sheet.scanning) {
                var all = true
                sheet.cats = sheet.cats.map(function (c) {
                    var s = Jobs.status(c.job)
                    var n = Object.assign({}, c)
                    if (s && s.id) { n.bytes = s.totalBytes || 0; n.done = s.state >= 5 }
                    all = all && n.done
                    return n
                })
                if (all) sheet.scanning = false
            }
            if (sheet.typesJob) {
                var t = Jobs.status(sheet.typesJob)
                if (t && t.id) {
                    if (t.info) sheet.typesInfo = t.info
                    if (t.state >= 5) {
                        if (t.state === 5 && t.info && t.info.done) {
                            sheet.typesTime = Date.now() / 1000
                            sheet.cachePut(t.info)
                        }
                        Jobs.forget(sheet.typesJob)
                        sheet.typesJob = 0
                    }
                }
            }
            if (sheet.checkJob) {
                var c = Jobs.status(sheet.checkJob)
                if (c && c.id) {
                    sheet.checkStatus = c
                    if (c.state >= 5) {
                        var r = c.info || {}
                        if (c.state === 6 && !r.result) r = { result: "failed", error: c.error }
                        if (c.state === 6 && !r.error) r.error = c.error
                        if (c.state === 7) r = null
                        sheet.checkResult = r
                        Jobs.forget(sheet.checkJob)
                        sheet.checkJob = 0
                        sheet.refreshInfo()
                    }
                }
            }
        }
    }

    // le fette in ordine; l'ultima è il libero
    readonly property var slices: {
        var out = [], sum = 0
        if (view === "types") {
            var by = typesInfo ? typesInfo.byType : null
            typeDefs.forEach(function (d) {
                var v = by && by[d.key]
                if (v && v.bytes > 0) { out.push({ key: d.key, name: d.name, color: d.color, bytes: v.bytes, count: v.count }); sum += v.bytes }
            })
            if (typesInfo && typesInfo.done) {
                // quello che la scansione non ha visto: cartelle senza permesso,
                // altri montaggi, spazio del file system stesso (giornale, inode)
                var rest = used - sum
                if (rest > used * 0.01)
                    out.push({ key: "", name: Style.t("Non contato", "Not counted"),
                              color: "#c3ccd8", bytes: rest, uncounted: true })
            } else if (!typesInfo) {
                out.push({ key: "", name: Style.t("Occupato", "Used"), color: "#4f8fe0", bytes: used })
            }
        } else {
            cats.forEach(function (c) { if (c.bytes > 0) { out.push({ key: "", name: c.name, color: c.color, bytes: c.bytes }); sum += c.bytes } })
            var r2 = Math.max(0, used - sum)
            out.push({ key: "", name: out.length ? Style.t("Altro (sistema, app, file sparsi)", "Other (system, apps, loose files)")
                                                 : Style.t("Occupato", "Used"), color: out.length ? "#c3ccd8" : "#4f8fe0", bytes: r2, uncounted: out.length > 0 })
        }
        out.push({ key: "", name: Style.t("Libero", "Free"), color: "rgba(160,172,186,0.13)", bytes: place.free || 0, free: true })
        return out
    }
    // nella legenda solo le fette che si vedono (le cartelle vuote davano «36 byte · 0,0%»)
    readonly property var legend: slices.filter(function (s) { return s.free || s.bytes >= 1e6 })
    onSlicesChanged: pie.requestPaint()

    function setView(v) {
        view = v
        expanded = ""
        if (v === "folders" && cats.length === 0) startScan()
    }

    function refreshInfo() {
        si = Fs.storageInfo(place.path || "/")
        load()
    }

    function openFor(p) {
        stopScan()
        stopTypes()
        place = p
        refreshInfo()
        expanded = ""
        checkResult = null
        checkStatus = {}
        showOutput = false
        var c = cacheGet()
        typesInfo = c ? c.info : null
        typesTime = c ? c.time : 0
        view = "types"
        // «/» intero dura minuti: lì solo col pulsante
        if (!c && p.kind !== 4) startTypes()
        homeFile = null
        if (p.kind === 4) {
            var hc = chainNodes(Fs.storageInfo(Fs.home).chain, [])
            for (var i = 0; i < hc.length; ++i)
                if (hc[i].kind === "loop" && hc[i].backingFile) homeFile = { path: hc[i].backingFile, size: hc[i].size }
        }
        open()
    }
    onClosed: { stopScan(); stopTypes() }

    function startCheck(repair) {
        checkResult = null
        showOutput = false
        checkJob = Jobs.fsCheck(si.mount, repair)
    }

    // ── righe ───────────────────────────────────────────────────────────────
    component Head: Text {
        x: Style.margin
        width: parent.width - 2 * Style.margin
        topPadding: 14 * Style.dp
        bottomPadding: 4 * Style.dp
        font.family: Style.fontFamily
        font.pixelSize: Style.fontS
        font.bold: true
        font.capitalization: Font.AllUppercase
        font.letterSpacing: 1.5 * Style.dp
        color: Style.accent
    }
    component Note: Text {
        x: Style.margin
        width: parent.width - 2 * Style.margin
        topPadding: 2 * Style.dp
        bottomPadding: 4 * Style.dp
        font.family: Style.fontFamily
        font.pixelSize: Style.fontS
        color: Style.dim
        wrapMode: Text.Wrap
    }
    component Field: Item {
        property string label
        property string value
        property string tone: ""
        width: parent.width
        height: Math.max(32 * Style.dp, v.implicitHeight + 10 * Style.dp)
        Text {
            id: l
            font.family: Style.fontFamily
            x: Style.margin
            width: 120 * Style.dp
            anchors.top: parent.top
            anchors.topMargin: 5 * Style.dp
            text: parent.label
            color: Style.dim
            font.pixelSize: Style.fontS
            wrapMode: Text.Wrap
        }
        Text {
            id: v
            font.family: Style.fontFamily
            anchors.left: l.right
            anchors.leftMargin: 6 * Style.dp
            anchors.right: parent.right
            anchors.rightMargin: Style.margin
            anchors.top: parent.top
            anchors.topMargin: 4 * Style.dp
            text: parent.value
            color: parent.tone === "danger" ? Style.danger : parent.tone === "amber" ? "#ffb547" : parent.tone === "ok" ? Style.ok : Style.text
            font.pixelSize: Style.fontM
            wrapMode: Text.Wrap
        }
    }
    component Tab: Rectangle {
        property string text
        property bool on: false
        signal clicked()
        width: tl.implicitWidth + 28 * Style.dp
        height: 34 * Style.dp
        radius: height / 2
        color: on ? Style.accentSoft : "transparent"
        border.width: 1.5 * Style.dp
        border.color: on ? Style.accent : Style.line
        Text { id: tl; anchors.centerIn: parent; text: parent.text; font.family: Style.fontFamily; font.pixelSize: Style.fontS; color: Style.text }
        MouseArea { anchors.fill: parent; onClicked: parent.clicked() }
    }

    // ── spazio ──────────────────────────────────────────────────────────────
    Head { text: Style.t("Spazio", "Space") }
    Item {
        width: parent.width
        height: 180 * Style.dp
        Canvas {
            id: pie
            width: 168 * Style.dp
            height: width
            anchors.centerIn: parent
            onWidthChanged: requestPaint()
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                var tot = sheet.place.total || 0
                if (!tot) return
                // la ciambella dentro, e fuori un anello sottile con tutto
                // l'occupato: la quota si legge anche quando una fetta grande
                // non è stata contata (utente 15/11: «24% sembra un ottavo»)
                var cx = width / 2, cy = height / 2, R = width / 2 - 1 * Style.dp
                var r = R - 9 * Style.dp, r2 = r * 0.62
                var a = -Math.PI / 2
                sheet.slices.forEach(function (s) {
                    if (s.bytes <= 0) return
                    var b = a + 2 * Math.PI * Math.min(1, s.bytes / tot)
                    ctx.beginPath()
                    ctx.arc(cx, cy, r, a, b, false)
                    ctx.arc(cx, cy, r2, b, a, true)
                    ctx.closePath()
                    ctx.fillStyle = s.color
                    ctx.fill()
                    if (s.uncounted) {
                        // righe oblique: occupato, ma non diviso per tipo
                        ctx.save()
                        ctx.clip()
                        ctx.strokeStyle = "rgba(60,72,88,0.55)"
                        ctx.lineWidth = 2 * Style.dp
                        for (var k = -width; k < width * 2; k += 7 * Style.dp) {
                            ctx.beginPath(); ctx.moveTo(k, 0); ctx.lineTo(k - height, height); ctx.stroke()
                        }
                        ctx.restore()
                    }
                    a = b
                })
                var used = Math.min(1, sheet.used / tot)
                ctx.beginPath()
                ctx.arc(cx, cy, R - 2.5 * Style.dp, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI, false)
                ctx.strokeStyle = "rgba(160,172,186,0.18)"
                ctx.lineWidth = 4 * Style.dp
                ctx.stroke()
                if (used > 0) {
                    ctx.beginPath()
                    ctx.arc(cx, cy, R - 2.5 * Style.dp, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * used, false)
                    ctx.strokeStyle = Style.accent
                    ctx.lineWidth = 4 * Style.dp
                    ctx.stroke()
                }
            }
        }
        Column {
            anchors.centerIn: pie
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                font.family: Style.fontFamily
                font.pixelSize: Style.fontL
                font.bold: true
                color: Style.text
                text: sheet.pct(sheet.used, sheet.place.total)
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                font.family: Style.fontFamily
                font.pixelSize: Style.fontS
                color: Style.dim
                text: Style.t("occupato", "used")
            }
        }
    }
    Field { label: Style.t("Totale", "Total"); value: Style.size(sheet.place.total || 0) }
    Field { label: Style.t("Occupato", "Used"); value: Style.size(sheet.used) + " (" + sheet.pct(sheet.used, sheet.place.total) + ")" }
    Field { label: Style.t("Libero", "Free"); value: Style.size(sheet.place.free || 0) + " (" + sheet.pct(sheet.place.free, sheet.place.total) + ")" }
    Field {
        visible: (sheet.si.reserved || 0) > 0
        label: Style.t("Riservato", "Reserved")
        value: Style.size(sheet.si.reserved || 0) + Style.t(" per root: conta come occupato, ma le app non lo possono usare", " for root: counted as used, but apps cannot use it")
    }
    Field {
        visible: !!sheet.homeFile
        label: Style.t("Nota", "Note")
        value: sheet.homeFile ? Style.t("l'occupato comprende il file della memoria interna cifrata (" + sheet.homeFile.path + ", " + Style.size(sheet.homeFile.size) + "): la memoria interna è dentro questo file",
                                        "used space includes the encrypted internal storage file (" + sheet.homeFile.path + ", " + Style.size(sheet.homeFile.size) + "): internal storage lives inside it") : ""
    }
    Field {
        visible: sheet.place.kind === 0 || sheet.place.kind === 1
        label: Style.t("Nota", "Note")
        value: sheet.place.kind === 0 ? Style.t("stessa partizione della Memoria Android: lo spazio è in comune", "same partition as Android storage: space is shared")
                                      : Style.t("stessa partizione della Memoria interna: lo spazio è in comune", "same partition as Internal storage: space is shared")
    }

    // per tipo | per cartella (su «/» le cartelle non si misurano)
    Row {
        x: Style.margin
        spacing: 8 * Style.dp
        topPadding: 6 * Style.dp
        bottomPadding: 6 * Style.dp
        visible: sheet.place.kind !== 4
        Tab { text: Style.t("Per tipo", "By type"); on: sheet.view === "types"; onClicked: sheet.setView("types") }
        Tab { text: Style.t("Per cartella", "By folder"); on: sheet.view === "folders"; onClicked: sheet.setView("folders") }
    }

    // la legenda della torta; per tipo, un tocco apre i file più grandi
    Repeater {
        model: sheet.legend
        delegate: Column {
            id: row
            width: parent ? parent.width : 0
            readonly property bool canOpen: sheet.view === "types" && !!modelData.key && !!sheet.typesInfo && !!sheet.typesInfo.largest
            readonly property bool isOpen: canOpen && sheet.expanded === modelData.key
            Item {
                width: parent.width
                height: 30 * Style.dp
                Rectangle { anchors.fill: parent; color: ma.pressed && row.canOpen ? Style.pressed : "transparent" }
                Rectangle {
                    id: sw
                    x: Style.margin
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14 * Style.dp; height: width; radius: 3 * Style.dp
                    color: modelData.free ? Style.alpha(Style.text, 0.13) : modelData.color
                    border.width: modelData.free || modelData.uncounted ? 1 : 0
                    border.color: Style.line
                }
                Text {
                    anchors.left: sw.right
                    anchors.leftMargin: 8 * Style.dp
                    anchors.right: sz.left
                    anchors.rightMargin: 8 * Style.dp
                    anchors.verticalCenter: parent.verticalCenter
                    font.family: Style.fontFamily
                    font.pixelSize: Style.fontM
                    color: Style.text
                    elide: Text.ElideRight
                    text: modelData.name + (row.canOpen ? (row.isOpen ? "  ▴" : "  ▾") : "")
                }
                Text {
                    id: sz
                    anchors.right: parent.right
                    anchors.rightMargin: Style.margin
                    anchors.verticalCenter: parent.verticalCenter
                    font.family: Style.fontFamily
                    font.pixelSize: Style.fontS
                    color: Style.dim
                    text: (modelData.count ? Style.count(modelData.count, Style.t("file", "file"), Style.t("file", "files")) + " · " : "")
                          + Style.size(modelData.bytes) + " · " + sheet.pct(modelData.bytes, sheet.place.total)
                }
                MouseArea {
                    id: ma
                    anchors.fill: parent
                    enabled: row.canOpen
                    onClicked: sheet.expanded = row.isOpen ? "" : modelData.key
                }
            }
            // i file più grandi di quel tipo: un tocco apre la loro cartella
            Repeater {
                model: row.isOpen ? (sheet.typesInfo.largest[modelData.key] || []) : []
                delegate: Item {
                    width: row.width
                    height: 44 * Style.dp
                    Rectangle { anchors.fill: parent; color: fa.pressed ? Style.pressed : "transparent" }
                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: Style.margin + 22 * Style.dp
                        anchors.right: fsz.left
                        anchors.rightMargin: 8 * Style.dp
                        anchors.verticalCenter: parent.verticalCenter
                        Text { width: parent.width; elide: Text.ElideMiddle; font.family: Style.fontFamily; font.pixelSize: Style.fontM; color: Style.text; text: sheet.baseName(modelData.path) }
                        Text { width: parent.width; elide: Text.ElideMiddle; font.family: Style.fontFamily; font.pixelSize: Style.fontS; color: Style.dim; text: sheet.dirName(modelData.path).replace(Fs.home, "~") }
                    }
                    Text {
                        id: fsz
                        anchors.right: parent.right
                        anchors.rightMargin: Style.margin
                        anchors.verticalCenter: parent.verticalCenter
                        font.family: Style.fontFamily
                        font.pixelSize: Style.fontS
                        color: Style.dim
                        text: Style.size(modelData.size)
                    }
                    MouseArea {
                        id: fa
                        anchors.fill: parent
                        onClicked: { var d = sheet.dirName(modelData.path); sheet.close(); sheet.openFolder(d) }
                    }
                }
            }
        }
    }

    // stato della misura per tipo, e il pulsante
    Note {
        visible: sheet.view === "types"
        text: {
            var i = sheet.typesInfo
            if (sheet.typesRunning)
                return Style.t("Conto i file… ", "Counting files… ") + (i ? Style.count(i.files, "file", "file") + " · " + Style.size(i.bytes) : "")
            if (!i)
                return sheet.place.kind === 4 ? Style.t("Sul file system intero la misura dura qualche minuto: parte col pulsante.", "On the whole file system measuring takes a few minutes: start it with the button.")
                                              : Style.t("Mai misurata.", "Never measured.")
            var s = Style.t("Misurata ", "Measured ") + sheet.ago(sheet.typesTime) + " · " + Style.count(i.files, "file", "file") + Style.t(", spazio vero su disco.", ", real space on disk.")
            s += sheet.place.kind === 0 ? Style.t(" «Non contato»: cartelle senza permesso, le altre cartelle di /home (di sistema e di altre app) e lo spazio del file system stesso.",
                                                  " “Not counted”: folders without permission, the other /home folders (system and other apps) and the file system's own space.")
                                        : Style.t(" «Non contato»: cartelle senza permesso e lo spazio del file system stesso.", " “Not counted”: folders without permission and the file system's own space.")
            if (i.unreadable > 0 && i.unreadableSample && i.unreadableSample.length)
                s += Style.t(" Senza permesso: ", " No permission: ") + i.unreadableSample.slice(0, 3).map(function (p) { return p.replace(Fs.home + "/", "~/") }).join(", ") + (i.unreadable > 3 ? "…" : "") + "."
            if (i.unreadable > 0)
                s += " " + (Fs.rootActive ? Style.t(i.unreadable + " cartelle non leggibili.", i.unreadable + " folders not readable.")
                                          : Style.t(i.unreadable + " cartelle si leggono solo con la modalità root (menu ⋮): sono nel «Non contato».",
                                                    i.unreadable + " folders are readable only in root mode (⋮ menu): they are in “Not counted”."))
            return s
        }
    }
    Row {
        x: Style.margin
        spacing: 8 * Style.dp
        bottomPadding: 4 * Style.dp
        visible: sheet.view === "types"
        SteelPill {
            text: sheet.typesRunning ? Style.t("Ferma", "Stop") : (sheet.typesInfo ? Style.t("Misura di nuovo", "Measure again") : Style.t("Misura", "Measure"))
            onClicked: sheet.typesRunning ? sheet.stopTypes() : sheet.startTypes()
        }
    }
    Note {
        visible: sheet.view === "folders"
        text: sheet.scanning ? Style.t("Misuro le cartelle…", "Measuring folders…") : Style.t("Le cartelle predefinite; «Altro» è tutto il resto.", "The standard folders; “Other” is everything else.")
    }

    // ── file system ─────────────────────────────────────────────────────────
    Head { text: Style.t("File system", "File system") }
    Field { label: Style.t("Tipo", "Type"); value: sheet.si.fstype || "?" }
    Field { label: Style.t("Montato su", "Mounted on"); value: sheet.si.mount || "?" }
    Field { visible: !!sheet.si.bindOf; label: Style.t("Cartella", "Folder"); value: Style.t("è la cartella ", "it is the folder ") + (sheet.si.bindOf || "") }
    Field { label: Style.t("Dispositivo", "Device"); value: (sheet.si.source || "?") + (sheet.si.device && (sheet.si.source || "").indexOf(sheet.si.device) < 0 ? " (" + sheet.si.device + ")" : "") }
    Field {
        label: Style.t("Cifratura", "Encryption")
        value: sheet.cryptNode ? sheet.cryptNode.crypt + Style.t(" (cifrata, si sblocca all'avvio)", " (encrypted, unlocked at boot)") : Style.t("nessuna", "none")
    }
    Field {
        visible: !!sheet.cryptNode || sheet.removable && !!sheet.place.label
        label: sheet.cryptNode ? "UUID LUKS" : Style.t("ID volume", "Volume ID")
        value: sheet.cryptNode ? (sheet.cryptNode.cryptUuid || "") : (sheet.place.label || "")
    }
    Field { label: Style.t("Accesso", "Access"); value: sheet.si.readOnly ? Style.t("sola lettura", "read-only") : Style.t("lettura e scrittura", "read and write"); tone: sheet.si.readOnly && sheet.place.kind !== 4 ? "danger" : "" }
    Field {
        visible: (sheet.si.inodesTotal || 0) > 0
        label: Style.t("File e cartelle", "Files and folders")
        value: Style.t(sheet.millions(sheet.si.inodesTotal - sheet.si.inodesFree) + " su " + sheet.millions(sheet.si.inodesTotal) + " possibili",
                       sheet.millions(sheet.si.inodesTotal - sheet.si.inodesFree) + " of " + sheet.millions(sheet.si.inodesTotal) + " possible")
    }
    Field { visible: (sheet.si.blockSize || 0) > 0; label: Style.t("Blocco", "Block"); value: Style.size(sheet.si.blockSize || 0) }
    Field {
        visible: !!sheet.si.trim
        label: "TRIM"
        value: {
            var t = sheet.si.trim || {}
            if (t.online) return Style.t("a ogni cancellazione (opzione discard)", "on every delete (discard option)")
            if (t.timerOn) return Style.t("periodico (fstrim.timer)", "periodic (fstrim.timer)")
            if (t.supported) return Style.t("la memoria lo supporta, ma non è attivo: il sistema non lo fa da solo", "supported by the storage, but not active: the system does not run it")
            return Style.t("non supportato", "not supported")
        }
    }
    Field { label: Style.t("Opzioni", "Options"); value: (sheet.si.options || "").split(",").join(", ") }

    // ── memoria fisica ──────────────────────────────────────────────────────
    Head { visible: sheet.phys.length > 0; text: Style.t("Memoria fisica", "Physical storage") }
    Repeater {
        model: sheet.phys
        delegate: Field { label: modelData.label; value: modelData.value; tone: modelData.tone || "" }
    }

    // ── stato ───────────────────────────────────────────────────────────────
    Head { text: Style.t("Stato", "Health") }
    Repeater {
        model: sheet.health
        delegate: Field { label: modelData.label; value: modelData.value; tone: modelData.tone || "" }
    }

    // ── controllo (schede e chiavette) ──────────────────────────────────────
    Head { visible: sheet.removable; text: Style.t("Controllo", "Check") }
    Note {
        visible: sheet.removable
        text: {
            if (sheet.si.checkable)
                return Style.t("Cerca gli errori del file system (" + sheet.si.fstype + "): file persi, cartelle rovinate, spazio contato male, di solito dopo una scheda tolta senza smontarla. Per il tempo del controllo la scheda viene smontata, poi si rimonta da sola. Il controllo non cambia niente: se trova errori, la riparazione si chiede a parte.",
                               "Looks for file system errors (" + sheet.si.fstype + "): lost files, broken folders, miscounted space, usually after removing a card without unmounting it. The card is unmounted during the check and mounted again afterwards. The check changes nothing: if it finds errors, repairing is a separate step.")
            switch (sheet.si.fstype) {
            case "exfat":
                return Style.t("Questa scheda è in exFAT, e su SailfishOS non c'è un programma che sappia controllarla (exfatprogs non è nei repository di Jolla): il telefono la legge e la scrive, ma non può cercarne gli errori. Per controllarla serve un computer: su Windows «chkdsk X: /f», su Linux «fsck.exfat», su Mac Utility Disco → S.O.S. Se la si formatta in ext4 (perdendo i dati), RooTree la può controllare da qui.",
                               "This card is exFAT, and SailfishOS has no program that can check it (exfatprogs is not in Jolla's repositories): the phone reads and writes it but cannot look for errors. To check it you need a computer: on Windows “chkdsk X: /f”, on Linux “fsck.exfat”, on Mac Disk Utility → First Aid. Formatted as ext4 (losing the data), RooTree can check it from here.")
            case "vfat":
            case "msdos":
                return Style.t("Per controllare una scheda FAT serve il pacchetto dosfstools di Jolla (da terminale: devel-su pkcon install dosfstools); poi il controllo si fa da qui.",
                               "Checking a FAT card needs Jolla's dosfstools package (from a terminal: devel-su pkcon install dosfstools); then the check runs from here.")
            }
            return Style.t("Su SailfishOS non c'è un programma per controllare " + (sheet.si.fstype || "questo file system") + ".", "SailfishOS has no program to check " + (sheet.si.fstype || "this file system") + ".")
        }
    }
    Note {
        visible: sheet.removable && !!sheet.si.checkable && !Fs.rootActive && sheet.checkJob === 0
        color: "#ffb547"
        text: Style.t("Serve la modalità root (menu ⋮ → Modalità root): il controllo legge la scheda direttamente.", "Needs root mode (⋮ menu → Root mode): the check reads the card directly.")
    }
    Row {
        x: Style.margin
        spacing: 8 * Style.dp
        bottomPadding: 4 * Style.dp
        visible: sheet.removable && !!sheet.si.checkable
        SteelPill {
            visible: sheet.checkJob === 0
            enabled: Fs.rootActive
            role: "accept"
            text: Style.t("Controlla la scheda", "Check the card")
            onClicked: confirmCheck.open()
        }
        SteelPill {
            visible: sheet.checkJob === 0 && !!sheet.checkResult && sheet.checkResult.result === "errors"
            enabled: Fs.rootActive
            role: "danger"
            text: Style.t("Ripara", "Repair")
            onClicked: confirmRepair.open()
        }
        SteelPill {
            visible: sheet.checkJob !== 0
            text: Style.t("Annulla", "Cancel")
            onClicked: Jobs.cancel(sheet.checkJob)
        }
    }
    Note {
        visible: sheet.checkJob !== 0
        color: Style.text
        text: {
            switch (sheet.checkStatus.current) {
            case "unmount": return Style.t("Smonto la scheda…", "Unmounting the card…")
            case "check": return Style.t("Controllo in corso: su una scheda grande può durare qualche minuto. ", "Checking: on a large card it can take a few minutes. ") + (sheet.checkStatus.dest || "")
            case "mount": return Style.t("Rimonto la scheda…", "Mounting the card again…")
            }
            return Style.t("Preparo il controllo…", "Preparing the check…")
        }
    }
    Field {
        visible: !!sheet.checkResult
        label: Style.t("Esito", "Result")
        readonly property var r: sheet.checkResult || {}
        tone: r.result === "clean" || r.result === "fixed" ? "ok" : "danger"
        value: {
            switch (r.result) {
            case "clean": return Style.t("nessun errore", "no errors")
            case "fixed": return Style.t("errori corretti", "errors fixed")
            case "errors": return r.repair ? Style.t("restano errori che il programma non sa correggere: copia altrove i dati e riformatta la scheda", "errors remain that the program cannot fix: copy the data elsewhere and reformat the card")
                                           : Style.t("trovati errori: si possono riparare col pulsante qui sopra", "errors found: they can be repaired with the button above")
            }
            return Style.t("non riuscito: ", "failed: ") + (r.error || Style.t("codice ", "code ") + r.code)
        }
    }
    Field {
        visible: !!sheet.checkResult && !!sheet.checkResult.mountError
        label: Style.t("Scheda", "Card")
        tone: "danger"
        value: Style.t("non si è rimontata: toglila e rimettila. ", "was not mounted again: remove and reinsert it. ") + ((sheet.checkResult || {}).mountError || "")
    }
    Text {
        x: Style.margin
        visible: !!sheet.checkResult && !!sheet.checkResult.output
        font.family: Style.fontFamily
        font.pixelSize: Style.fontS
        font.underline: true
        color: Style.accent
        topPadding: 4 * Style.dp
        bottomPadding: 6 * Style.dp
        text: sheet.showOutput ? Style.t("Nascondi i dettagli", "Hide details") : Style.t("Mostra i dettagli", "Show details")
        MouseArea { anchors.fill: parent; anchors.margins: -8 * Style.dp; onClicked: sheet.showOutput = !sheet.showOutput }
    }
    Text {
        x: Style.margin
        width: parent.width - 2 * Style.margin
        visible: sheet.showOutput && !!sheet.checkResult
        font.family: "monospace"
        font.pixelSize: Style.fontS * 0.85
        color: Style.dim
        wrapMode: Text.WrapAnywhere
        text: (sheet.checkResult || {}).output || ""
    }

    AppDialog {
        id: confirmCheck
        title: Style.t("Controllare la scheda?", "Check the card?")
        message: Style.t("Per il tempo del controllo la scheda viene smontata: chiudi prima i file aperti dalla scheda (musica, video, documenti). Il controllo non modifica niente.",
                         "The card is unmounted while it is checked: first close files opened from the card (music, videos, documents). The check changes nothing.")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Controlla", "Check"), role: "accept" }]
        onChosen: function (role) { if (role === "accept") sheet.startCheck(false) }
    }
    AppDialog {
        id: confirmRepair
        title: Style.t("Riparare la scheda?", "Repair the card?")
        message: Style.t("Il programma corregge gli errori che ha trovato. Di solito non si perde niente, ma i file rovinati possono essere spostati, troncati o tolti: se sulla scheda ci sono dati importanti che non hai altrove, copiali prima.",
                         "The program fixes the errors it found. Usually nothing is lost, but damaged files may be moved, truncated or removed: if the card holds important data you have nowhere else, copy it first.")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Ripara", "Repair"), role: "danger" }]
        onChosen: function (role) { if (role === "danger") sheet.startCheck(true) }
    }
}
