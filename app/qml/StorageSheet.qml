import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// La pagina della «i» di una memoria: spazio con la torta, file system,
// memoria fisica e stato. Tutto quello che non dà Fs.places() lo legge da
// /proc e /sys con Opener.readText (che legge anche i file a dimensione 0).
// La torta divide l'occupato per CARTELLE (le predefinite, misurate dal
// nucleo in background con Jobs.size); la divisione per tipo di file
// richiede una scansione nel nucleo (vedi proposta per il PC, 15/11/2026).
Sheet {
    id: sheet
    property var place: ({})
    property var fs: ({})      // file system e dispositivo
    property var phys: []      // memoria fisica: [{ label, value }]
    property var health: []    // stato: [{ label, value, warn }]
    property var cats: []      // fette: [{ name, color, dirs, job, bytes, done }]
    property bool scanning: false

    readonly property real used: Math.max(0, (place.total || 0) - (place.free || 0))

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

    // ── lettura di /proc e /sys ─────────────────────────────────────────────
    function rd(path) {
        var r = Opener.readText(path)
        return (r && r.text !== undefined) ? r.text.trim() : ""
    }
    function kv(text) {
        var o = {}
        text.split("\n").forEach(function (l) { var i = l.indexOf("="); if (i > 0) o[l.slice(0, i)] = l.slice(i + 1) })
        return o
    }
    // il punto di mount che contiene `path` (il più lungo), da mountinfo
    function mountOf(path) {
        var best = null
        rd("/proc/self/mountinfo").split("\n").forEach(function (l) {
            var sep = l.indexOf(" - ")
            if (sep < 0) return
            var a = l.slice(0, sep).split(" "), b = l.slice(sep + 3).split(" ")
            var mp = a[4].replace(/\\040/g, " ")
            var inside = path === mp || mp === "/" || path.indexOf(mp + "/") === 0
            if (inside && (!best || mp.length >= best.mount.length))
                best = { majmin: a[2], root: a[3], mount: mp, opts: a[5], type: b[0], source: b[1], superOpts: b[2] || "" }
        })
        return best
    }
    function pct(part, whole) {
        if (!whole) return ""
        var p = 100 * part / whole
        return Number(p).toLocaleString(Qt.locale(), 'f', p < 10 ? 1 : 0) + "%"
    }
    function luksUuid(u) {
        var m = /^CRYPT-(LUKS\d?)-([0-9a-f]{32})/.exec(u)
        if (!m) return null
        var h = m[2]
        return { kind: m[1], uuid: h.slice(0, 8) + "-" + h.slice(8, 12) + "-" + h.slice(12, 16) + "-" + h.slice(16, 20) + "-" + h.slice(20) }
    }
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

    function load() {
        var m = mountOf(place.path || "/")
        var f = { mount: m ? m.mount : "", type: m ? m.type : "", source: m ? m.source : "", ro: false, opts: "", dev: "", crypt: null, dmName: "" }
        var phy = [], st = []
        if (m) {
            f.ro = m.opts.split(",").indexOf("ro") >= 0
            // opzioni del mount e del superblocco, senza doppioni (rw compare in tutte e due)
            f.opts = (m.opts + (m.superOpts ? "," + m.superOpts : "")).split(",")
                     .filter(function (o, i, a) { return o && a.indexOf(o) === i }).join(",")
            var ue = kv(rd("/sys/dev/block/" + m.majmin + "/uevent"))
            f.dev = ue.DEVNAME || ""
            var dmName = rd("/sys/dev/block/" + m.majmin + "/dm/name")
            if (dmName) {
                f.dmName = dmName
                f.crypt = luksUuid(rd("/sys/dev/block/" + m.majmin + "/dm/uuid"))
            }
            if (m.root !== "/" && m.mount !== "/")
                f.bind = m.root
            // stato del file system: ext4 tiene il conto degli errori e dei dati scritti
            if (m.type === "ext4" && f.dev) {
                var e = "/sys/fs/ext4/" + f.dev + "/"
                var errs = parseInt(rd(e + "errors_count"))
                if (!isNaN(errs)) {
                    var txt = errs === 0 ? Style.t("nessuno", "none") : String(errs)
                    var t1 = parseInt(rd(e + "first_error_time")), t2 = parseInt(rd(e + "last_error_time"))
                    if (errs > 0 && t1) txt += Style.t(" (primo ", " (first ") + Style.date(t1) + Style.t(", ultimo ", ", last ") + Style.date(t2) + ")"
                    st.push({ label: Style.t("Errori registrati", "Recorded errors"), value: txt, warn: errs > 0 })
                }
                var life = parseInt(rd(e + "lifetime_write_kbytes")), sess = parseInt(rd(e + "session_write_kbytes"))
                if (!isNaN(life)) st.push({ label: Style.t("Scritti in tutto", "Written in total"), value: Style.size(life * 1024) + Style.t(" da quando è stato creato", " since creation") })
                if (!isNaN(sess)) st.push({ label: Style.t("Scritti ora", "Written now"), value: Style.size(sess * 1024) + Style.t(" da questa accensione", " since this boot") })
            } else if (m) {
                var onErr = /errors=([a-z-]+)/.exec(f.opts)
                st.push({ label: Style.t("Errori registrati", "Recorded errors"),
                          value: Style.t(m.type + " non ne tiene il conto", m.type + " does not count them")
                                 + (onErr ? Style.t(" (a un errore: ", " (on error: ") + onErr[1] + ")" : "") })
            }
        }
        st.push({ label: Style.t("Settori danneggiati", "Bad sectors"),
                  value: Style.t("le memorie flash sostituiscono da sole i blocchi guasti con quelli di riserva, e il sistema non li vede: a dire quanto è consumata è l'usura qui sopra (dove c'è). Per controllare una scheda in tutto si usa un PC.",
                                 "flash storage swaps failing blocks for reserve ones by itself, invisibly to the system: wear (above, where reported) tells how worn it is. A full card check needs a computer.") })

        // memoria fisica: scheda (mmcblk con type SD), eMMC, o UFS (sd*)
        var disk = f.dev.replace(/p\d+$/, "")
        if (f.dev.indexOf("mmcblk") === 0) {
            var d = "/sys/block/" + disk + "/device/"
            var type = rd(d + "type"), cap = parseInt(rd("/sys/block/" + disk + "/size")) * 512
            var mk = sdMakers[rd(d + "manfid")]
            var cls = type === "SD" ? (cap > 32e9 ? "SDXC" : (cap > 2.1e9 ? "SDHC" : "SD")) : (type === "MMC" ? "eMMC" : type)
            phy.push({ label: Style.t("Tipo", "Type"), value: cls })
            phy.push({ label: Style.t("Modello", "Model"), value: (mk ? mk + " " : "") + rd(d + "name") })
            if (cap) phy.push({ label: Style.t("Capacità", "Capacity"), value: Style.size(cap) })
            var date = rd(d + "date")
            if (date) phy.push({ label: Style.t("Prodotta", "Made"), value: date })
            var ser = rd(d + "serial")
            if (ser) phy.push({ label: Style.t("Numero di serie", "Serial"), value: ser })
        } else if (rd("/sys/block/sda/device/model")) {
            var chip = 0
            "abcdefgh".split("").forEach(function (c) { var s = parseInt(rd("/sys/block/sd" + c + "/size")); if (s) chip += s * 512 })
            phy.push({ label: Style.t("Tipo", "Type"), value: "UFS" + (f.dmName ? Style.t(" (memoria interna del telefono)", " (phone's internal storage)") : "") })
            phy.push({ label: Style.t("Modello", "Model"), value: rd("/sys/block/sda/device/vendor") + " " + rd("/sys/block/sda/device/model") })
            phy.push({ label: Style.t("Firmware", "Firmware"), value: rd("/sys/block/sda/device/rev") })
            if (chip) phy.push({ label: Style.t("Capacità del chip", "Chip capacity"), value: Style.size(chip) })
            var h = "/sys/devices/platform/bootdevice/health_descriptor/"
            var la = rd(h + "life_time_estimation_a"), lb = rd(h + "life_time_estimation_b"), eol = rd(h + "eol_info")
            // il chip dichiara l'usura a decine (0x02 = tra il 10 e il 20% della vita
            // prevista); A è l'area veloce, B il grosso della memoria
            if (la || lb) phy.push({ label: Style.t("Usura", "Wear"),
                                     value: (la === lb ? ufsLife(la) : "A " + ufsLife(la) + " · B " + ufsLife(lb))
                                            + Style.t(" della vita prevista (stima del chip, a scatti di 10%)", " of rated life (chip estimate, in 10% steps)") })
            if (eol) phy.push({ label: Style.t("Riserva blocchi", "Reserve blocks"), value: ufsEol(eol), warn: parseInt(eol, 16) >= 2 })
        }
        fs = f
        phys = phy
        health = st
    }

    // ── le fette per cartella ───────────────────────────────────────────────
    readonly property var catDefs: [
        { name: Style.t("Immagini", "Pictures"), color: "#4fa3ff", dirs: ["Pictures", "DCIM"] },
        { name: Style.t("Video", "Videos"), color: "#b66dff", dirs: ["Videos", "Movies"] },
        { name: Style.t("Musica", "Music"), color: "#ff6fae", dirs: ["Music", "Audiobooks", "Podcasts"] },
        { name: Style.t("Documenti", "Documents"), color: "#ffc247", dirs: ["Documents"] },
        { name: Style.t("Download", "Downloads"), color: "#5ccf8a", dirs: ["Downloads", "Download"] },
        // i dati delle app (Android/data, Android/obb) senza root non si leggono:
        // finiscono in «Altro», e la legenda lo dice
        { name: Fs.rootActive ? Style.t("Android", "Android") : Style.t("Android (senza dati app)", "Android (no app data)"), color: "#9ccc3c", dirs: ["android_storage"] },
        { name: Fs.rootActive ? Style.t("Dati delle app Android", "Android app data") : Style.t("Dati app Android (serve root)", "Android app data (needs root)"), color: "#9ccc3c", dirs: ["Android"] }
    ]
    function startScan() {
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
        enabled: sheet.scanning
        function onJobsChanged() {
            var all = true
            var c2 = sheet.cats.map(function (c) {
                var s = Jobs.status(c.job)
                var n = Object.assign({}, c)
                if (s && s.id) { n.bytes = s.totalBytes || 0; n.done = s.state >= 5 }
                all = all && n.done
                return n
            })
            sheet.cats = c2
            if (all) sheet.scanning = false
        }
    }
    // le fette in ordine: cartelle, poi il resto dell'occupato, poi il libero
    readonly property var slices: {
        var out = [], sum = 0
        cats.forEach(function (c) { if (c.bytes > 0) { out.push({ name: c.name, color: c.color, bytes: c.bytes }); sum += c.bytes } })
        var rest = Math.max(0, used - sum)
        out.push({ name: out.length ? Style.t("Altro (sistema, app, file sparsi)", "Other (system, apps, loose files)")
                                    : Style.t("Occupato", "Used"), color: out.length ? "#8a96a3" : "#4f8fe0", bytes: rest })
        out.push({ name: Style.t("Libero", "Free"), color: "rgba(160,172,186,0.28)", bytes: place.free || 0, free: true })
        return out
    }
    onSlicesChanged: pie.requestPaint()

    function openFor(p) {
        stopScan()
        place = p
        load()
        startScan()
        open()
    }
    onClosed: stopScan()

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
    component Field: Item {
        property string label
        property string value
        property bool warn: false
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
            color: parent.warn ? Style.danger : Style.text
            font.pixelSize: Style.fontM
            wrapMode: Text.Wrap
        }
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
                var cx = width / 2, cy = height / 2, r = width / 2 - 2 * Style.dp, r2 = r * 0.6
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
                    a = b
                })
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
        visible: sheet.place.kind === 0 || sheet.place.kind === 1
        label: Style.t("Nota", "Note")
        value: sheet.place.kind === 0 ? Style.t("stessa partizione della Memoria Android: lo spazio è in comune", "same partition as Android storage: space is shared")
                                      : Style.t("stessa partizione della Memoria interna: lo spazio è in comune", "same partition as Internal storage: space is shared")
    }

    // la legenda della torta
    Repeater {
        model: sheet.slices
        delegate: Item {
            width: parent ? parent.width : 0
            height: 28 * Style.dp
            Rectangle {
                id: sw
                x: Style.margin
                anchors.verticalCenter: parent.verticalCenter
                width: 14 * Style.dp; height: width; radius: 3 * Style.dp
                color: modelData.free ? Style.alpha(Style.text, 0.28) : modelData.color
                border.width: modelData.free ? 1 : 0
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
                text: modelData.name
            }
            Text {
                id: sz
                anchors.right: parent.right
                anchors.rightMargin: Style.margin
                anchors.verticalCenter: parent.verticalCenter
                font.family: Style.fontFamily
                font.pixelSize: Style.fontS
                color: Style.dim
                text: Style.size(modelData.bytes) + " · " + sheet.pct(modelData.bytes, sheet.place.total)
            }
        }
    }
    Text {
        x: Style.margin
        width: parent.width - 2 * Style.margin
        topPadding: 2 * Style.dp
        visible: sheet.cats.length > 0 || sheet.place.kind === 4
        font.family: Style.fontFamily
        font.pixelSize: Style.fontS
        color: Style.dim
        wrapMode: Text.Wrap
        text: sheet.place.kind === 4 ? Style.t("Per il file system intero non si misurano le cartelle: ci vorrebbe troppo.", "Folders are not measured for the whole file system: it would take too long.")
              : sheet.scanning ? Style.t("Misuro le cartelle…", "Measuring folders…")
              : Style.t("Divisione per cartella. La divisione per tipo di file arriverà con un aggiornamento.", "Split by folder. A split by file type will come with an update.")
    }

    // ── file system ─────────────────────────────────────────────────────────
    Head { text: Style.t("File system", "File system") }
    Field { label: Style.t("Tipo", "Type"); value: sheet.fs.type || "?" }
    Field { label: Style.t("Montato su", "Mounted on"); value: sheet.fs.mount || "?" }
    Field { visible: !!sheet.fs.bind; label: Style.t("Cartella", "Folder"); value: Style.t("collegamento a ", "bind of ") + (sheet.fs.bind || "") }
    Field { label: Style.t("Dispositivo", "Device"); value: (sheet.fs.source || "?") + (sheet.fs.dev && sheet.fs.source.indexOf(sheet.fs.dev) < 0 ? " (" + sheet.fs.dev + ")" : "") }
    Field {
        label: Style.t("Cifratura", "Encryption")
        value: sheet.fs.crypt ? sheet.fs.crypt.kind + Style.t(" (cifrata, si sblocca all'avvio)", " (encrypted, unlocked at boot)") : Style.t("nessuna", "none")
    }
    Field {
        visible: !!sheet.fs.crypt || (sheet.place.kind === 2 || sheet.place.kind === 3) && !!sheet.place.label
        label: sheet.fs.crypt ? "UUID LUKS" : Style.t("ID volume", "Volume ID")
        value: sheet.fs.crypt ? sheet.fs.crypt.uuid : (sheet.place.label || "")
    }
    Field { label: Style.t("Accesso", "Access"); value: sheet.fs.ro ? Style.t("sola lettura", "read-only") : Style.t("lettura e scrittura", "read and write"); warn: !!sheet.fs.ro && sheet.place.kind !== 4 }
    Field { label: Style.t("Opzioni", "Options"); value: (sheet.fs.opts || "").split(",").join(", ") }

    // ── memoria fisica ──────────────────────────────────────────────────────
    Head { visible: sheet.phys.length > 0; text: Style.t("Memoria fisica", "Physical storage") }
    Repeater {
        model: sheet.phys
        delegate: Field { label: modelData.label; value: modelData.value; warn: !!modelData.warn }
    }

    // ── stato ───────────────────────────────────────────────────────────────
    Head { text: Style.t("Stato", "Health") }
    Repeater {
        model: sheet.health
        delegate: Field { label: modelData.label; value: modelData.value; warn: !!modelData.warn }
    }
}
