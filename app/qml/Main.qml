import QtQuick
import QtQuick.Window
import QtQuick.Controls.Basic
import QtCore
import QtQuick.Effects
import RooTree
import "."

// RooTree: navigazione a cartelle come Dolphin. In alto la memoria (menù a
// tendina) e il percorso a briciole; al centro la cartella, a icone o a
// dettagli; in basso indietro/avanti/su, la vista e il menù, che diventano le
// azioni quando c'è una selezione.
Window {
    id: win
    visible: true
    color: Style.bg
    title: "RooTree"

    Rectangle {
        anchors.fill: parent
        visible: !Style.sailfish
        gradient: Gradient {
            GradientStop { position: 0; color: Style.bg2 }
            GradientStop { position: 1; color: Style.bg }
        }
    }
    // tema Sailfish: fondo pieno (l'immagine dell'ambience no, scelta
    // dell'utente del 27/10), è il colore della finestra

    // mai 0: una finestra che parte larga 0 (offscreen) darebbe divisioni per zero
    function updateDp() { if (width > 0 && height > 0) Style.dp = Math.min(width, height) / 400 }
    Component.onCompleted: {
        updateDp()
        loadAmbience()
        Fs.trashSetDays(settings.trashDays)
        Fs.trashPurge()
        openingSync.start()
        var s = sortOf(folder.path)
        folder.setSorting(s.k, s.d, s.f)
    }
    onWidthChanged: updateDp()
    onHeightChanged: updateDp()

    Settings {
        id: settings
        property string view: "grid"
        property bool thumbnails: true
        property alias showHidden: folder.showHidden
        // l'ordinamento di tutto il sistema; per data, dal più recente
        property int sortKey: 2
        property bool descending: false
        property bool dirsFirst: true
        // «Solo per questa cartella» (acceso di base) e gli ordinamenti delle
        // singole cartelle, {percorso: {k, d, f}} (scelta dell'utente, 08/11)
        property bool sortPerFolder: true
        property string folderSorts: "{}"
        property string theme: "rootundra"
        // il cestino si svuota da solo dopo questi giorni (0 = mai; scelta dell'utente del 12/11: 30)
        property int trashDays: 30
        onTrashDaysChanged: Fs.trashSetDays(trashDays)
    }

    // ── ordinamento: della cartella se ne ha uno suo, altrimenti di sistema ──
    function folderSortMap() {
        try { return JSON.parse(settings.folderSorts) || {} } catch (e) { return {} }
    }
    function sortOf(path) {
        var own = folderSortMap()[path]
        return own ? own : { k: settings.sortKey, d: settings.descending, f: settings.dirsFirst }
    }
    // cambiare cartella: prima il suo ordine, poi il percorso (una lettura sola)
    function setPath(path) {
        var s = sortOf(path)
        folder.setSorting(s.k, s.d, s.f)
        folder.path = path
    }
    function changeSort(k, d, f) {
        var m = folderSortMap()
        if (settings.sortPerFolder) {
            m[folder.path] = { k: k, d: d, f: f }
        } else {
            // per tutto il sistema: vale ovunque, anche dove c'era un ordine proprio
            settings.sortKey = k
            settings.descending = d
            settings.dirsFirst = f
            m = {}
        }
        settings.folderSorts = JSON.stringify(m)
        folder.setSorting(k, d, f)
    }
    function setSortPerFolder(on) {
        settings.sortPerFolder = on
        // spento: l'ordine che si vede ora diventa quello di tutto il sistema
        if (!on)
            changeSort(folder.sortKey, folder.descending, folder.dirsFirst)
    }
    // il tema e i colori dell'ambience; l'ambience si rilegge anche quando
    // l'app torna in primo piano (potrebbe essere cambiata nel frattempo)
    Binding { target: Style; property: "theme"; value: settings.theme }
    function loadAmbience() { if (Style.sailfish) Style.amb = Fs.ambience() }
    Connections {
        target: Style
        function onThemeChanged() { win.loadAmbience() }
    }
    Connections {
        target: Qt.application
        function onStateChanged() { if (Qt.application.state === Qt.ApplicationActive) win.loadAmbience() }
    }

    FolderModel {
        id: folder
        path: startPath
    }

    readonly property bool selecting: folder.selectedCount > 0
    property var places: Fs.places()
    property var back: []
    property var forward: []

    // la memoria che contiene `path`: quella col percorso più lungo che lo
    // contiene (la radice vale solo se non c'è altro)
    readonly property var place: {
        // su un server: la «memoria» è la connessione (la barra del percorso
        // parte dal suo nome)
        if (folder.remote !== "") {
            var rp = Remotes.get(folder.remote)
            return { kind: 5, path: "/@remote/" + folder.remote, label: rp.name || "?", icon: Remotes.typeInfo(rp.type).icon || "cloud", total: 0, free: 0 }
        }
        var best = null
        for (var i = 0; i < places.length; ++i) {
            var p = places[i]
            // una chiavetta non montata non ha percorso: non contiene niente
            if (p.mounted === false)
                continue
            var inside = folder.path === p.path || p.path === "/" || folder.path.indexOf(p.path + "/") === 0
            if (inside && (!best || p.path.length > best.path.length))
                best = p
        }
        return best || { kind: 4, path: "/", label: "" }
    }
    function placeName(p) {
        switch (p.kind) {
        case 0: return Style.t("Memoria interna", "Internal storage")
        case 1: return Style.t("Memoria Android", "Android storage")
        case 2: return Style.t("Scheda SD", "SD card")
        case 3: return p.label || "USB"
        case 5: return p.label
        }
        return "/"
    }
    function placeIcon(p) { return p.kind === 5 ? p.icon : ["internal", "android", "sdcard", "usb", "root"][p.kind] }

    function go(path) {
        if (path === folder.path)
            return
        back = back.concat([folder.path])
        forward = []
        folder.clearSelection()
        setPath(path)
    }
    function goBack() {
        if (back.length === 0) return
        forward = [folder.path].concat(forward)
        folder.clearSelection()
        setPath(back[back.length - 1])
        back = back.slice(0, -1)
    }
    function goForward() {
        if (forward.length === 0) return
        back = back.concat([folder.path])
        folder.clearSelection()
        setPath(forward[0])
        forward = forward.slice(1)
    }
    // su un server non si sale oltre la sua radice
    readonly property bool canGoUp: folder.path !== "/" && !(folder.remote !== "" && folder.path === win.place.path)
    function goUp() { if (canGoUp) go(Fs.parentOf(folder.path)) }

    // toccare un file: le immagini e il testo nei visori di RooTree, il resto
    // con l'app predefinita di Sailfish, o «Apri con» se non ce n'è una
    function activate(index) {
        var e = folder.get(index)
        if (selecting) {
            folder.toggleSelected(index)
            return
        }
        if (e.isDir) {
            go(e.path)
            return
        }
        // un archivio si sfoglia come una cartella (Estrai qui è nel menù)
        if (Fs.isArchive(e.name)) {
            go(e.path)
            return
        }
        if (folder.archive !== "") {
            openArchived(e)
            return
        }
        if (folder.remote !== "") {
            openRemote(e)
            return
        }
        openFile(e)
    }
    // un file su un server: il testo si legge (e si modifica) sul server; il
    // resto si scarica nella cache e si apre da lì, con la stessa coda (si
    // vede l'avanzamento e si può annullare)
    property var pendingRemote: ({})
    function openRemote(e) {
        var mime = Opener.mimeType(e.name)
        if (e.category === "text" || e.category === "code" || mime === "text/plain") {
            textViewer.openFor(e.path)
            return
        }
        downloadThen([e.path], "open", e)
    }
    function downloadThen(paths, action, entry) {
        var dir = Fs.remoteCacheDir(paths[0])
        var id = Jobs.copy(paths, dir)
        var p = Object.assign({}, pendingRemote)
        p[id] = { action: action, dir: dir, names: paths.map(function (x) { return x.split("/").pop() }), entry: entry || ({}) }
        pendingRemote = p
        toast.show(Style.t("Scarico dal server…", "Downloading from the server…"))
    }
    // un download per aprire o condividere è finito: true = gestito qui
    function remoteDone(id, state, error) {
        var r = pendingRemote[id]
        if (!r) return false
        var p = Object.assign({}, pendingRemote); delete p[id]; pendingRemote = p
        Jobs.forget(id)
        if (state === 6) toast.show(Style.t("Download non riuscito: ", "Download failed: ") + error, true)
        if (state !== 5) return true
        var files = r.names.map(function (n) { return Fs.join(r.dir, n) })
        if (r.action === "share") {
            if (!Opener.share(files) && Opener.lastError()) win.showSandboxError()
        } else if (r.entry.category === "image") {
            imageViewer.openAt([{ path: files[0], name: r.entry.name, size: r.entry.size, mtime: r.entry.mtime }], 0)
        } else {
            openFile({ path: files[0], name: r.entry.name, category: r.entry.category })
        }
        return true
    }

    // un file dentro un archivio: si estrae nella cache e si apre da lì
    function openArchived(e) {
        var p = Fs.extractForOpen(e.path)
        if (!p) {
            toast.show(Style.t("Impossibile aprire: ", "Cannot open: ") + Fs.lastError(), true)
            return
        }
        if (e.category === "image")
            imageViewer.openAt([{ path: p, name: e.name, size: e.size, mtime: e.mtime }], 0)
        else
            openFile({ path: p, name: e.name, category: e.category })
    }
    function openFile(e) {
        var mime = Opener.mimeType(e.path)
        if (e.category === "image" && mime !== "image/svg+xml")
            openImages(e.path)
        else if (e.category === "text" || e.category === "code" || mime === "text/plain")
            textViewer.openFor(e.path)
        else if (!Opener.openDefault(e.path)) {
            if (mime.indexOf("text/") === 0)
                textViewer.openFor(e.path)
            else
                openWith.openFor(e.path)
        } else if (Opener.lastError())
            win.showSandboxError()
    }
    // le app di Sailfish (nella sandbox) vedono solo Documenti, Download, Musica,
    // Immagini, Pubblica, Video e le schede: un file grande altrove non lo si può
    // passare loro senza copiarlo
    function showSandboxError() {
        toast.show(Style.t("Il file è troppo grande per passarlo da qui alle app di Sailfish: spostalo in Documenti, Download, Musica, Immagini o Video.",
                           "The file is too big to hand over to Sailfish apps from here: move it to Documents, Downloads, Music, Pictures or Videos."), true)
    }
    // tutte le immagini della cartella, nell'ordine della vista, per scorrerle
    function openImages(path) {
        var list = []
        var at = 0
        for (var i = 0; i < folder.count; ++i) {
            var e = folder.get(i)
            if (e.category !== "image")
                continue
            if (e.path === path)
                at = list.length
            list.push({ path: e.path, name: e.name, size: e.size, mtime: e.mtime })
        }
        imageViewer.openAt(list, at)
    }

    // Elimina: nel cestino, o definitivo con la casella (scelta dell'utente del 12/11)
    function deleteSelected() {
        var list = folder.selectedPaths()
        confirm.paths = list
        confirm.hint = Fs.trashHint(list)
        forever.checked = false
        confirm.open()
    }

    // ── in alto: memoria + percorso ─────────────────────────────────────────
    SteelPanel {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: -border.width; rightMargin: -border.width; topMargin: -border.width }
        height: Style.bar + 4 * Style.dp
        // il filo LED in basso solo in selezione (altrimenti c'è già quello
        // della briciola dove si è)
        led: win.selecting ? "bottom" : ""
        ledWidth: width
        z: 2

        // la memoria: un disco d'acciaio con la freccia del menù a tendina
        IconButton {
            id: placeButton
            anchors { left: parent.left; leftMargin: 2 * Style.dp; verticalCenter: parent.verticalCenter }
            width: Style.bar + 14 * Style.dp
            height: Style.bar
            discScale: 0.86
            discShift: -7 * Style.dp
            visible: !win.selecting
            icon: win.placeIcon(win.place)
            checked: placesMenu.opened
            onClicked: placesMenu.open()
            Icon { anchors { right: parent.right; verticalCenter: parent.verticalCenter } name: "down"; size: 11 * Style.dp; opacity: 0.8 }
        }
        PathBar {
            anchors { left: placeButton.right; leftMargin: 4 * Style.dp; right: parent.right; top: parent.top; bottom: parent.bottom }
            visible: !win.selecting
            path: folder.path
            rootPath: win.place.path
            rootLabel: win.placeName(win.place)
            onNavigate: function (p) { win.go(p) }
        }

        // in selezione: quanti e la X per uscire
        IconButton {
            id: endSelection
            visible: win.selecting
            anchors { left: parent.left; verticalCenter: parent.verticalCenter }
            icon: "close"
            onClicked: folder.clearSelection()
        }
        Text {
            font.family: Style.fontFamily
            visible: win.selecting
            anchors { left: endSelection.right; verticalCenter: parent.verticalCenter }
            text: Style.t(folder.selectedCount === 1 ? "1 selezionato" : folder.selectedCount + " selezionati",
                          folder.selectedCount + " selected")
            color: Style.text
            font.pixelSize: Style.fontL
        }
        IconButton {
            visible: win.selecting
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            icon: "selectall"
            onClicked: folder.selectedCount === folder.count ? folder.clearSelection() : folder.selectAll()
        }
    }

    // ── modalità root: una fascia rossa che non si può non vedere ───────────
    Rectangle {
        id: rootBand
        visible: Fs.rootActive
        anchors { left: parent.left; right: parent.right; top: header.bottom }
        height: visible ? 30 * Style.dp : 0
        color: "#c62828"
        z: 1
        Icon { id: rootIcon; anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter } name: "root"; size: 16 * Style.dp; tint: "white" }
        Text {
            font.family: Style.fontFamily
            anchors { left: rootIcon.right; leftMargin: 8 * Style.dp; right: rootOff.left; verticalCenter: parent.verticalCenter }
            text: Style.t("MODALITÀ ROOT: le modifiche toccano il sistema", "ROOT MODE: changes affect the system")
            color: "white"
            font.pixelSize: Style.fontS
            font.bold: true
            elide: Text.ElideRight
        }
        Text {
            id: rootOff
            font.family: Style.fontFamily
            anchors { right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
            text: Style.t("Spegni", "Turn off")
            color: "white"
            font.pixelSize: Style.fontS
            font.underline: true
        }
        MouseArea { anchors.fill: parent; onClicked: win.rootOff() }
    }
    function rootOff() {
        Fs.rootStop()
        folder.refresh()
        toast.show(Style.t("Modalità root spenta", "Root mode off"))
    }
    Connections {
        target: Fs
        function onRootFinished(code, message) {
            if (code === "") {
                rootDialog.close()
                folder.refresh()
                toast.show(Style.t("Modalità root accesa", "Root mode on"))
            } else {
                rootDialog.error = code === "AUTH"
                    ? Style.t("Password rifiutata. È quella di Impostazioni → Strumenti per sviluppatori.",
                              "Password rejected. It is the one in Settings → Developer tools.")
                    : message
                rootDialog.open()
            }
        }
    }

    // ── la cartella ─────────────────────────────────────────────────────────
    Item {
        id: content
        anchors { left: parent.left; right: parent.right; top: rootBand.bottom; bottom: status.top }
        clip: true

        GridView {
            id: grid
            visible: settings.view === "grid"
            anchors.fill: parent
            anchors.margins: 4 * Style.dp
            model: visible ? folder : null
            readonly property int columns: Math.max(3, Math.floor(width / (92 * Style.dp)))
            cellWidth: width / columns
            cellHeight: cellWidth * 0.72 + 50 * Style.dp
            boundsBehavior: Flickable.StopAtBounds
            delegate: Item {
                width: grid.cellWidth
                height: grid.cellHeight
                // la selezione: un riquadro col bordo LED, come l'evidenziazione di Dolphin
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 3 * Style.dp
                    radius: 10 * Style.dp
                    visible: selected || gridArea.pressed
                    color: selected ? Style.selected : Style.pressed
                    border.color: selected ? Style.accent : "transparent"
                    border.width: 2 * Style.dp
                }
                FileIcon {
                    id: gi
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 6 * Style.dp
                    width: grid.cellWidth * 0.72
                    height: width
                    path: model.path
                    category: model.category
                    thumbnails: settings.thumbnails && !folder.readOnly && folder.remote === ""
                    opacity: hidden ? 0.5 : 1
                    scale: gridArea.pressed ? 0.94 : 1
                    Behavior on scale { NumberAnimation { duration: 80 } }
                }
                Icon {
                    visible: isLink
                    anchors { right: gi.right; bottom: gi.bottom }
                    name: "link"
                    size: gi.size * 0.32
                }
                Text {
                    font.family: Style.fontFamily
                    anchors { top: gi.bottom; topMargin: 4 * Style.dp; left: parent.left; right: parent.right; margins: 4 * Style.dp }
                    horizontalAlignment: Text.AlignHCenter
                    text: name
                    color: selected ? Style.text : (hidden ? Style.dim : Style.text)
                    font.pixelSize: 13 * Style.dp
                    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                    maximumLineCount: 2
                    elide: Text.ElideMiddle
                }
                MouseArea {
                    id: gridArea
                    anchors.fill: parent
                    onClicked: win.activate(index)
                    onPressAndHold: folder.toggleSelected(index)
                }
            }
        }

        ListView {
            id: list
            visible: settings.view === "list"
            anchors.fill: parent
            model: visible ? folder : null
            boundsBehavior: Flickable.StopAtBounds
            delegate: Rectangle {
                width: list.width
                height: Style.row
                color: selected ? Style.selected : (listArea.pressed ? Style.pressed : "transparent")
                FileIcon {
                    id: li
                    anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                    width: 44 * Style.dp
                    height: width
                    small: true
                    path: model.path
                    category: model.category
                    thumbnails: settings.thumbnails && !folder.readOnly && folder.remote === ""
                    opacity: hidden ? 0.5 : 1
                }
                Icon {
                    visible: isLink
                    anchors { right: li.right; bottom: li.bottom }
                    name: "link"
                    size: 13 * Style.dp
                }
                Column {
                    anchors { left: li.right; leftMargin: Style.margin; right: sizeText.left; rightMargin: 8 * Style.dp; verticalCenter: parent.verticalCenter }
                    Text { font.family: Style.fontFamily; width: parent.width; text: name; color: hidden ? Style.dim : Style.text; font.pixelSize: Style.fontM; elide: Text.ElideMiddle }
                    Text { font.family: Style.fontFamily; width: parent.width; text: Style.date(mtime); color: Style.dim; font.pixelSize: Style.fontS }
                }
                Text {
                    font.family: Style.fontFamily
                    id: sizeText
                    anchors { right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                    text: isDir ? "" : Style.size(size)
                    color: Style.dim
                    font.pixelSize: Style.fontS
                }
                Rectangle { anchors { left: li.right; right: parent.right; bottom: parent.bottom } height: 1; color: Style.line; opacity: 0.6 }
                MouseArea {
                    id: listArea
                    anchors.fill: parent
                    onClicked: win.activate(index)
                    onPressAndHold: folder.toggleSelected(index)
                }
            }
        }

        // vuota o non leggibile
        Column {
            anchors.centerIn: parent
            width: parent.width * 0.8
            spacing: 8 * Style.dp
            visible: folder.count === 0
            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                width: 76 * Style.dp; height: width
                source: folder.error ? "" : "icons/tipi/folder.png"
                sourceSize: Qt.size(width, height)
                opacity: 0.45
                layer.enabled: Style.sailfish
                layer.effect: MultiEffect { colorization: 1; colorizationColor: Style.sfHighlight }
                Icon { visible: folder.error !== ""; anchors.centerIn: parent; name: "lock"; size: 48 * Style.dp }
            }
            Text {
                font.family: Style.fontFamily
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: folder.error ? Style.t("Impossibile aprire questa cartella", "Cannot open this folder") : Style.t("Cartella vuota", "Empty folder")
                color: Style.text
                font.pixelSize: Style.fontL
            }
            Text {
                font.family: Style.fontFamily
                visible: folder.error !== ""
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: folder.error + (folder.error.indexOf("ermission") >= 0
                      ? "\n" + Style.t("Per le cartelle di sistema c'è la modalità root, nel menù ⋮.", "For system folders there is root mode, in the ⋮ menu.") : "")
                color: Style.dim
                font.pixelSize: Style.fontS
            }
        }
    }

    // ── la scheda d'avanzamento: un'operazione è partita e si vede ──────────
    ProgressCard {
        anchors { left: parent.left; right: parent.right; bottom: status.top; leftMargin: 8 * Style.dp; rightMargin: 8 * Style.dp; bottomMargin: 4 * Style.dp }
        z: 3
        onOpenJobs: jobsSheet.open()
    }

    // ── riga di stato, come la barra di stato di Dolphin ────────────────────
    Text {
        font.family: Style.fontFamily
        id: status
        anchors { left: parent.left; right: parent.right; bottom: pasteBar.visible ? pasteBar.top : bottomBar.top; leftMargin: Style.margin; rightMargin: Style.margin }
        height: 24 * Style.dp
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        color: Style.dim
        font.pixelSize: Style.fontS
        text: {
            var s = Style.count(folder.dirCount, Style.t("cartella", "folder"), Style.t("cartelle", "folders")) + ", "
                  + Style.count(folder.fileCount, "file", Style.t("file", "files"))
            if (folder.fileCount > 0)
                s += " (" + Style.size(folder.filesSize) + ")"
            if (win.place.total > 0)
                s += " · " + Style.freeOf(win.place.free, win.place.total)
            return s
        }
    }

    // ── Incolla qui ─────────────────────────────────────────────────────────
    SteelPanel {
        id: pasteBar
        visible: Jobs.clipboard.length > 0 && !win.selecting
        anchors { left: parent.left; right: parent.right; bottom: bottomBar.top; leftMargin: -border.width; rightMargin: -border.width }
        height: Style.bar + 4 * Style.dp
        ledWidth: width
        Icon { id: pi; anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter } name: Jobs.clipboardCut ? "cut" : "copy"; size: 20 * Style.dp }
        Text {
            font.family: Style.fontFamily
            anchors { left: pi.right; leftMargin: Style.margin; right: pasteButton.left; verticalCenter: parent.verticalCenter }
            text: (Jobs.clipboardCut ? Style.t("Da spostare: ", "To move: ") : Style.t("Da copiare: ", "To copy: "))
                  + (Jobs.clipboard.length === 1 ? Jobs.clipboard[0].split("/").pop() : Style.count(Jobs.clipboard.length, Style.t("elemento", "item"), Style.t("elementi", "items")))
            color: Style.text
            font.pixelSize: Style.fontS
            elide: Text.ElideMiddle
        }
        SteelPill {
            id: pasteButton
            anchors { right: clearClip.left; verticalCenter: parent.verticalCenter }
            role: "accept"
            text: Style.t("Incolla qui", "Paste here")
            enabled: Fs.isWritable(folder.path)
            onClicked: Jobs.paste(folder.path)
        }
        IconButton { id: clearClip; anchors { right: parent.right; verticalCenter: parent.verticalCenter } icon: "close"; onClicked: Jobs.clearClipboard() }
    }

    // ── in basso: navigazione o azioni sulla selezione ─────────────────────
    SteelPanel {
        id: bottomBar
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: -border.width; rightMargin: -border.width; bottomMargin: -border.width }
        height: (win.selecting ? Style.bar + 18 * Style.dp : Style.bar) + 10 * Style.dp
        led: ""

        // l'avanzamento della prima operazione aperta, come un filo sul bordo
        readonly property var running: Jobs.jobs.filter(function (j) { return j.kind !== 3 && j.kind < 7 && j.state < 5 })[0]
        Rectangle {
            visible: !!bottomBar.running
            anchors { left: parent.left; top: parent.top }
            height: 3 * Style.dp
            color: Style.accent
            width: !bottomBar.running ? 0 : parent.width * (bottomBar.running.totalBytes > 0 ? bottomBar.running.doneBytes / bottomBar.running.totalBytes
                                          : (bottomBar.running.totalItems > 0 ? bottomBar.running.doneItems / bottomBar.running.totalItems : 0))
        }

        Row {
            visible: !win.selecting
            anchors.fill: parent
            anchors.topMargin: 5 * Style.dp
            anchors.bottomMargin: 5 * Style.dp
            readonly property real w: width / 7
            IconButton { width: parent.w; height: parent.height; icon: "back"; enabled: win.back.length > 0; onClicked: win.goBack() }
            IconButton { width: parent.w; height: parent.height; icon: "forward"; enabled: win.forward.length > 0; onClicked: win.goForward() }
            IconButton { width: parent.w; height: parent.height; icon: "up"; enabled: win.canGoUp; onClicked: win.goUp() }
            IconButton { width: parent.w; height: parent.height; icon: "search"; enabled: !folder.readOnly && folder.remote === ""; onClicked: searchPanel.openIn(folder.path) }
            IconButton { width: parent.w; height: parent.height; icon: settings.view === "grid" ? "list" : "grid"; onClicked: settings.view = settings.view === "grid" ? "list" : "grid" }
            IconButton {
                width: parent.w; height: parent.height
                icon: "jobs"
                opacity: Jobs.jobs.some(function (j) { return j.kind !== 3 && j.kind < 7 }) ? 1 : 0.35
                onClicked: jobsSheet.open()
                Rectangle {
                    visible: Jobs.activeCount > 0
                    x: parent.width / 2 + parent.disc * 0.22
                    y: 8 * Style.dp
                    width: 17 * Style.dp; height: width; radius: width / 2
                    color: Style.accent
                    Text { font.family: Style.fontFamily; anchors.centerIn: parent; text: Jobs.activeCount; color: Style.text; font.pixelSize: 10 * Style.dp; font.bold: true }
                }
            }
            IconButton { width: parent.w; height: parent.height; icon: "menu"; onClicked: mainMenu.open() }
        }

        Row {
            visible: win.selecting
            anchors.fill: parent
            anchors.topMargin: 5 * Style.dp
            anchors.bottomMargin: 5 * Style.dp
            readonly property real w: width / 5
            IconButton { width: parent.w; height: parent.height; icon: "copy"; label: Style.t("Copia", "Copy"); onClicked: { Jobs.setClipboard(folder.selectedPaths(), false); folder.clearSelection() } }
            IconButton { width: parent.w; height: parent.height; icon: "cut"; label: Style.t("Taglia", "Cut"); enabled: !folder.readOnly; onClicked: { Jobs.setClipboard(folder.selectedPaths(), true); folder.clearSelection() } }
            IconButton {
                width: parent.w; height: parent.height
                icon: "share"; label: Style.t("Condividi", "Share")
                // si condividono i file, non le cartelle
                enabled: !folder.readOnly && folder.selectedCount > 0 && folder.selectedPaths().every(function (p) { return !Fs.info(p).isDir })
                onClicked: {
                    if (folder.remote !== "") win.downloadThen(folder.selectedPaths(), "share")
                    else if (!Opener.share(folder.selectedPaths()) && Opener.lastError()) win.showSandboxError()
                    folder.clearSelection()
                }
            }
            IconButton { width: parent.w; height: parent.height; icon: "delete"; label: Style.t("Elimina", "Delete"); enabled: !folder.readOnly; onClicked: win.deleteSelected() }
            IconButton {
                width: parent.w; height: parent.height
                icon: "menu"; label: Style.t("Altro", "More")
                onClicked: moreMenu.open()
            }
        }
    }

    // ── menù, pannelli, dialoghi ────────────────────────────────────────────
    PlacesMenu {
        id: placesMenu
        y: header.height - 2 * Style.dp
        currentPath: folder.path
        onNavigate: function (p) { win.go(p) }
        onClosed: win.places = places
        onAddConnection: wizard.openNew()
        onOpenConnection: function (p) { win.connectTo(p) }
        onConnectionActions: function (p) { connectionMenu.profile = p; connectionMenu.open() }
        onPlaceInfo: function (p) { storageSheet.openFor(p) }
        onMountRequested: function (p) { media.mount(p) }
        onEjectRequested: function (p) { media.eject(p) }
    }
    // ── chiavette e schede: monta ed espelli (0.9.7) ─────────────────────────
    Timer {
        id: ejectLater
        property string mount
        interval: 600
        onTriggered: Jobs.eject(mount)
    }
    // Sailfish OS monta da solo solo la microSD: le chiavette USB compaiono
    // «non montate» fra le Altre memorie e si montano al tocco.
    QtObject {
        id: media
        function name(p) {
            return p.kind === 2 ? Style.t("la scheda", "the card") : Style.t("la chiavetta", "the drive")
        }
        function mount(p) {
            Jobs.mount(p.device)
            toast.show(Style.t("Monto ", "Mounting ") + name(p) + "…")
        }
        function eject(p) {
            toast.show(Style.t("Espello ", "Ejecting ") + name(p) + "…")
            // la cartella aperta non deve tenerla occupata: si torna alla
            // Memoria interna e si espelle un attimo dopo, quando elenco e
            // anteprime della cartella lasciata hanno chiuso i loro file
            if (folder.path === p.path || folder.path.indexOf(p.path + "/") === 0) {
                win.go(Fs.home)
                ejectLater.mount = p.path
                ejectLater.restart()
                return
            }
            Jobs.eject(p.path)
        }
        function finished(id, kind, state, error) {
            var st = Jobs.status(id)
            var info = (st && st.info) || {}
            if (kind === 10) {
                if (state === 5) {
                    toast.show(Style.t("Montata", "Mounted"))
                    if (info.mountedAt)
                        win.go(info.mountedAt)
                } else if (state === 6) {
                    if (error.indexOf("MEMORIA:") === 0)
                        toast.show(Fs.rootActive
                                   ? Style.t("Il sistema non trova abbastanza memoria libera per montarla: riavvia il telefono e riprova.",
                                             "The system cannot find enough free memory to mount it: restart the phone and try again.")
                                   : Style.t("Il sistema non trova abbastanza memoria libera per montarla: accendi la modalità root e riprova (RooTree libera la memoria da solo), oppure riavvia il telefono.",
                                             "The system cannot find enough free memory to mount it: turn on root mode and try again (RooTree frees the memory by itself), or restart the phone."), true)
                    else
                        toast.show(Style.t("Non riesco a montarla: ", "Cannot mount it: ") + error, true)
                }
            } else {
                if (state === 5) {
                    toast.show(info.bus === "usb" && info.powerOff
                               ? Style.t("Chiavetta spenta: ora puoi toglierla", "Drive powered off: you can now remove it")
                               : Style.t("Smontata: ora puoi toglierla", "Unmounted: you can now remove it"))
                } else if (state === 6) {
                    if (error.indexOf("IN USO:") === 0) {
                        var who = error.substring(7)
                        if (who === "RooTree")
                            toast.show(Style.t("RooTree sta ancora leggendo dalla chiavetta (anteprime): riprova tra qualche secondo.",
                                               "RooTree is still reading from the drive (previews): try again in a few seconds."), true)
                        else
                            toast.show(Style.t("Non si può espellere: la sta usando " + who + ". Chiudi i file aperti dalla chiavetta e riprova.",
                                               "Cannot eject: " + who + " is using it. Close the files opened from the drive and try again."), true)
                    }
                    else
                        toast.show(Style.t("Non riesco a espellerla: ", "Cannot eject it: ") + error, true)
                }
            }
            if (state >= 5)
                Jobs.forget(id)
            win.places = Fs.places()
        }
    }
    StorageSheet {
        id: storageSheet
        onOpenFolder: function (p) { win.go(p) }
    }

    // ── connessioni remote ──────────────────────────────────────────────────
    ConnectionWizard {
        id: wizard
        onSaved: function (id) {
            toast.show(Style.t("Connessione salvata: la trovi nel menù delle memorie", "Connection saved: find it in the storage menu"))
            // «Ricorda nella cassaforte»: la password appena scritta ci va subito
            if (Remotes.get(id).remember !== false && Remotes.hasSecret(id))
                win.withVault(function () { win.rememberSecret(id) })
        }
    }

    // la cassaforte aperta (o creata, la prima volta) prima di `then`
    function withVault(then) {
        if (Remotes.vaultUnlocked) { then(); return }
        vaultDialog.ask(Remotes.vaultExists ? "unlock" : "create", then)
    }
    function rememberSecret(id) {
        var err = Remotes.remember(id)
        if (err) toast.show(Style.t("Password non salvata nella cassaforte: ", "Password not saved in the vault: ") + err, true)
    }

    // il collegamento: password o sblocco se servono, impronta del server, poi dentro
    function connectTo(p) {
        if (Remotes.isConnected(p.id) && Remotes.entryPath(p.id) !== "") {
            go(Remotes.entryPath(p.id))
            return
        }
        // cloud con accesso dal browser: il segreto è il token di rinnovo
        if (p.auth === "oauth" && !Remotes.hasSecret(p.id)) {
            if (p.remember !== false && Remotes.vaultExists && !Remotes.vaultUnlocked) {
                vaultDialog.ask("unlock", function () { win.connectTo(p) }, function () { loginDialog.ask(p) })
                return
            }
            loginDialog.ask(p)
            return
        }
        if ((p.auth === "password" || p.auth === "token") && !Remotes.hasSecret(p.id)) {
            // salvata nella cassaforte chiusa: prima si apre
            if (p.remember !== false && Remotes.vaultExists && !Remotes.vaultUnlocked) {
                vaultDialog.ask("unlock", function () { win.connectTo(p) }, function () { secretDialog.ask(p, p.auth, "") })
                return
            }
            secretDialog.ask(p, p.auth, "")
            return
        }
        doConnect(p)
    }
    function doConnect(p) {
        if (Remotes.connecting(p.id)) return
        toast.show(Style.t("Collegamento a «" + p.name + "»…", "Connecting to “" + p.name + "”…"))
        Remotes.connect(p.id)
    }
    Connections {
        target: Remotes
        function onMegaRevoked(id, error) {
            if (error === "")
                toast.show(Style.t("Fatto: al prossimo collegamento Mega chiederà di nuovo il codice.", "Done: Mega will ask for the code again at the next connection."))
            else
                toast.show(Style.t("Tolta da questo telefono, ma Mega non ha risposto: chiudi la sessione anche dal sito di Mega. ", "Removed from this phone, but Mega did not answer: close the session on the Mega site too. ") + error, true)
        }
        function onConnectFinished(id, code, message, path) {
            // aperta per una sincronizzazione (o per sfogliarne la cartella): si resta dove si è
            if (syncer.connectDone(id, code, message))
                return
            var p = Remotes.get(id)
            if (code === "") {
                win.go(path)
                return
            }
            var parts = message.split(":")
            if (code === "HOSTKEY_NEW") {
                hostKeyDialog.profile = p
                hostKeyDialog.keyType = parts[0]
                hostKeyDialog.fingerprint = parts.slice(1).join(":")
                hostKeyDialog.open()
            } else if (code === "HOSTKEY_CHANGED") {
                hostChanged.profile = p
                hostChanged.keyType = parts[0]
                hostChanged.fingerprint = parts.slice(1).join(":")
                hostChanged.open()
            } else if ((code === "AUTH" || code === "LOGIN") && p.auth === "oauth") {
                // accesso scaduto o revocato: si rifà dal browser
                Remotes.forgetSecret(id)
                loginDialog.ask(p, Style.t("L'accesso a " + (Remotes.typeInfo(p.type).label || "") + " è scaduto o è stato revocato: rifallo.", "The sign-in has expired or been revoked: do it again."))
            } else if (code === "MFA") {
                secretDialog.ask(p, "mfa", message === "wrong" ? Style.t("Codice sbagliato o scaduto: riprova.", "Wrong or expired code: try again.") : "")
            } else if (code === "AUTH" && p.auth === "token") {
                secretDialog.ask(p, "token", Style.t("Token rifiutato: " + message, "Token rejected: " + message))
            } else if (code === "AUTH" && p.auth === "password") {
                secretDialog.ask(p, "password", Style.t("Password rifiutata dal server.", "Password rejected by the server."))
            } else if (code === "KEY" && message.indexOf("frase") >= 0) {
                secretDialog.ask(p, "passphrase", "")
            } else if (code === "DRIVEID") {
                // kDrive col solo permesso «drive»: l'ID del drive lo scrive l'utente
                toast.show(message, true)
                wizard.openFor(p)
            } else if (code === "AUTH") {
                connectError.show(p, Style.t("Il server ha rifiutato la chiave", "The server rejected the key"), message)
            } else if (code === "NET") {
                connectError.show(p, Style.t("Server irraggiungibile", "Server unreachable"), message)
            } else {
                connectError.show(p, Style.t("Collegamento non riuscito", "Connection failed"), message)
            }
        }
    }

    // l'accesso dal browser (cloud): si apre il browser, si aspetta il ritorno
    AppDialog {
        id: loginDialog
        property var profile: ({})
        property bool waiting: false
        property string error
        function ask(p, err) {
            profile = p
            error = err || ""
            waiting = false
            open()
        }
        title: Style.t("Accedi a ", "Sign in to ") + (Remotes.typeInfo(profile.type || "").label || "")
        message: waiting
            ? Style.t("Accedi nel browser e autorizza RooTree, poi torna qui: il collegamento parte da solo.", "Sign in in the browser and authorise RooTree, then come back: the connection starts by itself.")
            : Style.t("Si aprirà il browser per accedere a «" + (profile.name || "") + "». La password la scrivi solo lì, non in RooTree.",
                      "The browser will open to sign in to “" + (profile.name || "") + "”. You type the password only there, not in RooTree.")
        buttons: waiting ? [{ text: Style.t("Annulla l'accesso", "Cancel sign-in"), role: "cancel" }]
                         : [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Apri il browser", "Open the browser"), role: "accept" }]
        onChosen: function (role) {
            if (role === "accept") {
                waiting = true
                Remotes.authorize(profile.id)
                open() // resta aperta in attesa
            } else if (role === "cancel") {
                Remotes.cancelAuth(profile.id)
                waiting = false
            }
        }
        Text { font.family: Style.fontFamily; visible: loginDialog.error !== ""; width: parent.width; wrapMode: Text.Wrap; text: loginDialog.error; color: Style.danger; font.pixelSize: Style.fontS }
    }
    Connections {
        target: Remotes
        function onAuthUrl(id, url) {
            if (!Opener.openUrl(url))
                toast.show(Style.t("Non riesco ad aprire il browser", "Cannot open the browser"), true)
        }
        function onAuthFinished(id, code, message) {
            var p = Remotes.get(id)
            loginDialog.waiting = false
            loginDialog.close()
            if (code === "") {
                toast.show(Style.t("Accesso riuscito", "Signed in"))
                if (p.remember !== false)
                    win.withVault(function () { win.rememberSecret(id); win.doConnect(p) })
                else
                    win.doConnect(p)
            } else if (code === "CANCELLED") {
                toast.show(message)
            } else if (code === "CLIENTID") {
                connectError.show(p, Style.t("Manca la chiave dell'app", "The app key is missing"),
                                  Style.t("Apri «Modifica la connessione» e incolla la App key: lì trovi anche come crearla.", "Open “Edit the connection” and paste the App key: you also find how to create it there."))
            } else {
                connectError.show(p, Style.t("Accesso non riuscito", "Sign-in failed"), message)
            }
        }
    }

    // la password (o la frase della chiave) al collegamento
    AppDialog {
        id: secretDialog
        property var profile: ({})
        property string what: "password"
        property string error
        function ask(p, kind, err) {
            profile = p
            what = kind
            error = err || ""
            secretField.text = ""
            rememberCheck.checked = p.remember !== false
            trustCheck.checked = false
            open()
            secretField.forceActiveFocus()
        }
        title: profile.name || ""
        message: what === "mfa"
            ? Style.t("Codice a 6 cifre della verifica in due passaggi di Mega (lo trovi nella tua app di autenticazione).", "6-digit two-step verification code for Mega (from your authenticator app).")
            : what === "token"
            ? Style.t("Token dell'API di kDrive (si crea nel Manager di Infomaniak, permesso «drive»).", "kDrive API token (created in the Infomaniak Manager, “drive” scope).")
            : what === "passphrase"
            ? Style.t("La chiave «" + (profile.keyPath || "").split("/").pop() + "» è protetta da una frase.", "The key “" + (profile.keyPath || "").split("/").pop() + "” is protected by a passphrase.")
            : Style.t("Password di " + Remotes.describe(profile), "Password for " + Remotes.describe(profile))
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Collega", "Connect"), role: "accept" }]
        onChosen: function (role) {
            if (role !== "accept" || secretField.text === "") return
            var p = profile
            if (what === "mfa") {
                Remotes.setOneTimeCode(p.id, secretField.text)
                secretField.text = ""
                var trust = trustCheck.checked
                if (trust !== (p.mfaTrusted === true)) {
                    p = Object.assign({}, p); p.mfaTrusted = trust; Remotes.save(p)
                }
                // la sessione va nella cassaforte: che sia aperta prima
                if (trust) win.withVault(function () { win.doConnect(p) })
                else win.doConnect(p)
                return
            }
            Remotes.setSecret(p.id, secretField.text)
            secretField.text = ""
            if (rememberCheck.checked !== (p.remember !== false)) {
                p = Object.assign({}, p); p.remember = rememberCheck.checked; Remotes.save(p)
            }
            if (rememberCheck.checked)
                win.withVault(function () { win.rememberSecret(p.id); win.doConnect(p) })
            else
                win.doConnect(p)
        }
        TextField {
            id: secretField
            font.family: Style.fontFamily
            width: parent.width
            color: Style.text
            font.pixelSize: Style.fontM
            echoMode: secretDialog.what === "mfa" ? TextInput.Normal : TextInput.Password
            inputMethodHints: secretDialog.what === "mfa" ? Qt.ImhDigitsOnly : (Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase)
            background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: secretField.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
            onAccepted: { secretDialog.close(); secretDialog.chosen("accept") }
        }
        CheckRow {
            id: trustCheck
            visible: secretDialog.what === "mfa"
            text: Style.t("Non chiedere più il codice su questo telefono", "Don't ask for the code again on this phone")
        }
        CheckRow { id: rememberCheck; visible: secretDialog.what !== "mfa"; text: Style.t("Ricorda nella cassaforte", "Remember in the vault") }
        Text { font.family: Style.fontFamily; visible: secretDialog.error !== ""; width: parent.width; wrapMode: Text.Wrap; text: secretDialog.error; color: Style.danger; font.pixelSize: Style.fontS }
    }

    // la cassaforte: crearla la prima volta, aprirla una volta per sessione
    AppDialog {
        id: vaultDialog
        property string mode: "unlock"
        property var then
        property var otherwise
        property string error
        // chi la chiede, se non è un collegamento fatto dall'utente (le sincronizzazioni all'apertura)
        property string reason
        function ask(m, fn, fallback, why) {
            mode = m
            then = fn
            otherwise = fallback || null
            reason = why || ""
            error = ""
            vaultPass.text = ""
            vaultPass2.text = ""
            open()
            vaultPass.forceActiveFocus()
        }
        title: mode === "create" ? Style.t("Crea la cassaforte", "Create the vault") : Style.t("Apri la cassaforte", "Open the vault")
        message: mode === "create"
            ? Style.t("Le password delle connessioni vengono salvate cifrate con una password principale, che scegli ora e che RooTree non salva. La chiederà una volta, quando serve, finché l'app resta aperta. Se la dimentichi, le password salvate sono perse (le connessioni restano).",
                      "Connection passwords are stored encrypted with a master password, which you choose now and RooTree does not store. It is asked once, when needed, while the app stays open. If you forget it, the saved passwords are lost (the connections stay).")
            : (reason ? reason + "\n\n" : "") + Style.t("Password principale della cassaforte di RooTree.", "Master password of the RooTree vault.")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: mode === "create" ? Style.t("Crea", "Create") : Style.t("Apri", "Open"), role: "accept" }]
        onChosen: function (role) {
            if (role !== "accept") {
                if (otherwise) otherwise()
                return
            }
            if (mode === "create" && vaultPass.text !== vaultPass2.text) {
                error = Style.t("Le due password non coincidono.", "The two passwords do not match.")
                open()
                return
            }
            var err = mode === "create" ? Remotes.vaultCreate(vaultPass.text) : Remotes.vaultUnlock(vaultPass.text)
            vaultPass.text = ""
            vaultPass2.text = ""
            if (err) {
                error = err.indexOf("WRONG") === 0 ? Style.t("Password principale sbagliata.", "Wrong master password.") : err
                open()
                return
            }
            if (then) then()
        }
        TextField {
            id: vaultPass
            font.family: Style.fontFamily
            width: parent.width
            color: Style.text
            font.pixelSize: Style.fontM
            echoMode: TextInput.Password
            placeholderText: Style.t("Password principale", "Master password")
            placeholderTextColor: Style.dim
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
            background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: vaultPass.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
            onAccepted: if (vaultDialog.mode !== "create") { vaultDialog.close(); vaultDialog.chosen("accept") }
        }
        TextField {
            id: vaultPass2
            visible: vaultDialog.mode === "create"
            font.family: Style.fontFamily
            width: parent.width
            color: Style.text
            font.pixelSize: Style.fontM
            echoMode: TextInput.Password
            placeholderText: Style.t("Ripetila", "Repeat it")
            placeholderTextColor: Style.dim
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
            background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: vaultPass2.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
            onAccepted: { vaultDialog.close(); vaultDialog.chosen("accept") }
        }
        Text { font.family: Style.fontFamily; visible: vaultDialog.error !== ""; width: parent.width; wrapMode: Text.Wrap; text: vaultDialog.error; color: Style.danger; font.pixelSize: Style.fontS }
    }

    // server mai visto: l'impronta da confermare, come fa ssh
    AppDialog {
        id: hostKeyDialog
        property var profile: ({})
        property string keyType
        property string fingerprint
        // FTPS: un certificato che nessuna autorità garantisce (normale per un NAS)
        readonly property bool tls: keyType === "TLS"
        title: tls ? Style.t("Certificato da confermare", "Certificate to confirm") : Style.t("Server nuovo", "New server")
        message: tls
            ? Style.t("Il certificato di «" + (profile.host || "") + "» non è garantito da un'autorità riconosciuta: per un NAS o un server di casa è normale. Se vuoi essere sicuro che sia davvero il tuo server, confronta questa impronta SHA-256 con quella del suo certificato (openssl x509 -noout -fingerprint -sha256 -in certificato.pem). RooTree la ricorderà.",
                      "The certificate of “" + (profile.host || "") + "” is not vouched for by a recognised authority: for a NAS or a home server that is normal. To be sure it really is your server, compare this SHA-256 fingerprint with its certificate's (openssl x509 -noout -fingerprint -sha256 -in certificate.pem). RooTree will remember it.")
            : Style.t("È la prima volta che RooTree si collega a «" + (profile.host || "") + "». Se vuoi essere sicuro che sia davvero il tuo server, confronta questa impronta con quella del server (ssh-keygen -lf sulla sua chiave).",
                         "This is the first time RooTree connects to “" + (profile.host || "") + "”. To be sure it really is your server, compare this fingerprint with the server's (ssh-keygen -lf on its key).")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Fidati e collega", "Trust and connect"), role: "accept" }]
        onChosen: function (role) {
            if (role !== "accept") return
            var err = Remotes.trust(profile.id)
            if (err) { connectError.show(profile, Style.t("Impronta non salvata", "Fingerprint not saved"), err); return }
            win.doConnect(profile)
        }
        Text {
            font.family: "monospace"
            width: parent.width
            wrapMode: Text.WrapAnywhere
            text: (hostKeyDialog.tls ? Style.t("Certificato TLS (SHA-256)", "TLS certificate (SHA-256)") : hostKeyDialog.keyType) + "\n" + hostKeyDialog.fingerprint
            color: Style.accent
            font.pixelSize: Style.fontS
        }
    }
    // impronta cambiata: nessuna scorciatoia
    AppDialog {
        id: hostChanged
        property var profile: ({})
        property string keyType
        property string fingerprint
        readonly property bool tls: keyType === "TLS"
        title: Style.t("Attenzione: il server è cambiato", "Warning: the server has changed")
        message: tls
            ? Style.t("Il certificato di «" + (profile.host || "") + "» non è quello che avevi confermato. Può essere un certificato rinnovato, oppure qualcuno che si spaccia per il server: RooTree non si collega. Se sai che è cambiato davvero, togli la riga di «" + (profile.host || "") + ":" + (profile.port || 21) + "» dal file ~/.config/com.github.RootGPT_YouTube/rootree/certificati-ftps.json (anche con l'editor di RooTree) e riprova.",
                      "The certificate of “" + (profile.host || "") + "” is not the one you confirmed. It may be a renewed certificate, or someone impersonating the server: RooTree does not connect. If you know it really changed, remove the line of “" + (profile.host || "") + ":" + (profile.port || 21) + "” from ~/.config/com.github.RootGPT_YouTube/rootree/certificati-ftps.json (the RooTree editor works too) and try again.")
            : Style.t("L'impronta di «" + (profile.host || "") + "» non è quella salvata in ~/.ssh/known_hosts. Può essere un server reinstallato, oppure qualcuno che si spaccia per lui: RooTree non si collega. Se sai che è cambiato davvero, togli la sua riga da known_hosts (ssh-keygen -R) e riprova.",
                         "The fingerprint of “" + (profile.host || "") + "” is not the one saved in ~/.ssh/known_hosts. It may be a reinstalled server, or someone impersonating it: RooTree does not connect. If you know it really changed, remove its line from known_hosts (ssh-keygen -R) and try again.")
        buttons: [{ text: "OK", role: "reject" }]
        Text {
            font.family: "monospace"
            width: parent.width
            wrapMode: Text.WrapAnywhere
            text: (hostChanged.tls ? Style.t("Certificato TLS (SHA-256)", "TLS certificate (SHA-256)") : hostChanged.keyType) + "\n" + hostChanged.fingerprint
            color: Style.danger
            font.pixelSize: Style.fontS
        }
    }
    AppDialog {
        id: connectError
        function show(p, t, m) { title = (p.name ? p.name + ": " : "") + t; message = m; open() }
        buttons: [{ text: "OK", role: "reject" }]
    }
    Sheet {
        id: connectionMenu
        property var profile: ({})
        title: profile.name || ""
        MenuRow {
            icon: "connect"
            visible: connectionMenu.opened && Remotes.isConnected(connectionMenu.profile.id || "")
            text: Style.t("Scollega", "Disconnect")
            onClicked: {
                connectionMenu.close()
                var id = connectionMenu.profile.id
                if (folder.remote === id) win.go(Fs.home)
                Remotes.disconnect(id)
                toast.show(Style.t("Scollegato", "Disconnected"))
            }
        }
        MenuRow {
            icon: "delete"
            visible: connectionMenu.opened && Remotes.isConnected(connectionMenu.profile.id || "")
            text: Style.t("Cestino", "Trash")
            onClicked: { connectionMenu.close(); trashSheet.openFor(connectionMenu.profile) }
        }
        MenuRow {
            icon: "rename"
            text: Style.t("Rinomina", "Rename")
            onClicked: {
                connectionMenu.close()
                var prof = connectionMenu.profile
                nameDialog.ask(Style.t("Nome della connessione", "Connection name"), prof.name || "", function (name) {
                    if (name === "") return Style.t("Il nome non può essere vuoto", "The name cannot be empty")
                    var c = Object.assign({}, prof)
                    c.name = name
                    Remotes.save(c)
                    return ""
                })
            }
        }
        MenuRow { icon: "openwith"; text: Style.t("Modifica", "Edit"); onClicked: { connectionMenu.close(); wizard.openFor(connectionMenu.profile) } }
        MenuRow { icon: "copy"; text: Style.t("Duplica", "Duplicate"); onClicked: { connectionMenu.close(); Remotes.duplicate(connectionMenu.profile.id) } }
        MenuRow {
            icon: "delete"
            text: Style.t("Elimina", "Delete")
            onClicked: { connectionMenu.close(); deleteConnection.profile = connectionMenu.profile; deleteConnection.open() }
        }
    }
    AppDialog {
        id: deleteConnection
        property var profile: ({})
        title: Style.t("Eliminare la connessione?", "Delete the connection?")
        readonly property int syncCount: profile.id ? Syncs.forRemote(profile.id).length : 0
        message: Style.t("«" + (profile.name || "") + "» sparirà dal menù. I file sul server non vengono toccati.",
                         "“" + (profile.name || "") + "” will disappear from the menu. Files on the server are not touched.")
                 + (syncCount === 1 ? Style.t(" Anche la sincronizzazione che la usa verrà eliminata.", " The sync using it will be deleted too.")
                    : syncCount > 1 ? Style.t(" Anche le " + syncCount + " sincronizzazioni che la usano verranno eliminate.", " The " + syncCount + " syncs using it will be deleted too.") : "")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Elimina", "Delete"), role: "danger" }]
        onChosen: function (role) {
            if (role !== "danger") return
            var l = Syncs.forRemote(profile.id)
            for (var i = 0; i < l.length; ++i) Syncs.remove(l[i].id)
            Remotes.remove(profile.id)
        }
    }

    Sheet {
        id: mainMenu
        // «Vista» e «Ordina per» in una tendina sola (richiesta dell'utente del
        // 15/11: il menù era troppo lungo), come «Opzioni per Remoto»
        MenuRow {
            id: viewOptions
            property bool open: false
            icon: settings.view === "grid" ? "grid" : "list"
            text: Style.t("Visualizzazione", "View")
            detail: open ? "" : (settings.view === "grid" ? Style.t("Icone", "Icons") : Style.t("Dettagli", "Details")) + " · "
                    + [Style.t("Nome", "Name"), Style.t("Dimensione", "Size"), Style.t("Data", "Date"), Style.t("Tipo", "Type")][folder.sortKey]
            expandable: true
            expanded: open
            onClicked: open = !open
            onToggled: open = !open
            Connections {
                target: mainMenu
                function onAboutToShow() { viewOptions.open = false }
            }
        }
        MenuRow { depth: 1; visible: viewOptions.open; icon: "grid"; text: Style.t("Icone", "Icons"); checkable: true; checked: settings.view === "grid"; onClicked: { settings.view = "grid"; mainMenu.close() } }
        MenuRow { depth: 1; visible: viewOptions.open; icon: "list"; text: Style.t("Dettagli", "Details"); checkable: true; checked: settings.view === "list"; onClicked: { settings.view = "list"; mainMenu.close() } }
        MenuRow { depth: 1; visible: viewOptions.open; icon: folder.showHidden ? "shown" : "hidden"; text: Style.t("Mostra i file nascosti", "Show hidden files"); checkable: true; checked: folder.showHidden; onClicked: folder.showHidden = !folder.showHidden }
        MenuRow { depth: 1; visible: viewOptions.open; icon: "image"; text: Style.t("Mostra le anteprime", "Show previews"); checkable: true; checked: settings.thumbnails; onClicked: settings.thumbnails = !settings.thumbnails }
        MenuSection { visible: viewOptions.open; depth: 1; text: Style.t("Ordina per", "Sort by") }
        Repeater {
            model: [Style.t("Nome", "Name"), Style.t("Dimensione", "Size"), Style.t("Data di modifica", "Modified"), Style.t("Tipo", "Type")]
            delegate: MenuRow { depth: 1; visible: viewOptions.open; text: modelData; checkable: true; checked: folder.sortKey === index; onClicked: win.changeSort(index, folder.descending, folder.dirsFirst) }
        }
        MenuRow {
            depth: 1
            visible: viewOptions.open
            icon: "sort"
            // per data l'ordine normale è dal più recente: l'inverso lo dice
            text: folder.sortKey === 2 ? Style.t("Prima i più vecchi", "Oldest first") : Style.t("Ordine inverso", "Descending")
            checkable: true; checked: folder.descending
            onClicked: win.changeSort(folder.sortKey, !folder.descending, folder.dirsFirst)
        }
        MenuRow { depth: 1; visible: viewOptions.open; icon: "folder"; text: Style.t("Cartelle prima", "Folders first"); checkable: true; checked: folder.dirsFirst; onClicked: win.changeSort(folder.sortKey, folder.descending, !folder.dirsFirst) }
        MenuRow {
            depth: 1
            visible: viewOptions.open
            icon: "check"
            text: Style.t("Solo per questa cartella", "Only for this folder")
            checkable: true; checked: settings.sortPerFolder
            onClicked: win.setSortPerFolder(!settings.sortPerFolder)
        }
        // fuori da «Ordina per» (richiesta dell'utente del 12/11)
        MenuSection { text: Style.t("Remoto", "Remote") }
        // «Opzioni per Remoto» (richiesta dell'utente dell'08/11): tutto ciò che
        // riguarda le connessioni in una sottosezione; aperta da sola quando
        // si è dentro un server, chiusa altrimenti
        MenuRow {
            id: remoteOptions
            property bool open: false
            icon: "network"
            text: Style.t("Opzioni per Remoto", "Remote options")
            detail: folder.remote !== "" ? (Remotes.get(folder.remote).name || "") : ""
            expandable: true
            expanded: open
            onClicked: open = !open
            onToggled: open = !open
            Connections {
                target: mainMenu
                function onAboutToShow() { remoteOptions.open = folder.remote !== "" }
            }
        }
        MenuRow {
            depth: 1
            icon: "home"
            visible: remoteOptions.open && folder.remote !== ""
            readonly property string serverPath: folder.remote === "" ? "" : (folder.path.substring(("/@remote/" + folder.remote).length) || "/")
            readonly property bool already: folder.remote !== "" && (Remotes.get(folder.remote).path || "") === serverPath
            // già quella: spunta e non grigio (grigia sembrava rotta, 08/11)
            text: already ? Style.t("Questa è la cartella iniziale", "This is the start folder") : Style.t("Apri sempre da questa cartella", "Always open from this folder")
            checkable: already
            checked: already
            onClicked: {
                mainMenu.close()
                if (already) {
                    toast.show(Style.t("«" + Remotes.get(folder.remote).name + "» si apre già da qui. Entra in un'altra cartella per cambiarla.",
                                       "“" + Remotes.get(folder.remote).name + "” already opens from here. Go into another folder to change it."))
                    return
                }
                Remotes.setStartPath(folder.remote, serverPath)
                toast.show(Style.t("«" + Remotes.get(folder.remote).name + "» si aprirà da " + serverPath, "“" + Remotes.get(folder.remote).name + "” will open from " + serverPath))
            }
        }
        MenuRow {
            depth: 1
            icon: "key"
            visible: remoteOptions.open && folder.remote !== "" && Remotes.get(folder.remote).mfaTrusted === true
            text: Style.t("Chiedi di nuovo il codice di Mega", "Ask for the Mega code again")
            onClicked: {
                mainMenu.close()
                var p = Object.assign({}, Remotes.get(folder.remote))
                win.withVault(function () {
                    p.mfaTrusted = false
                    Remotes.save(p)
                    win.go(Fs.home)
                    Remotes.disconnect(p.id)
                    Remotes.megaRevoke(p.id)
                })
            }
        }
        MenuRow {
            depth: 1
            icon: "rename"
            visible: remoteOptions.open && folder.remote !== ""
            text: Style.t("Modifica la connessione", "Edit the connection")
            onClicked: { mainMenu.close(); wizard.openFor(Remotes.get(folder.remote)) }
        }
        MenuRow {
            depth: 1
            icon: "connect"
            visible: remoteOptions.open && folder.remote !== ""
            text: Style.t("Scollega", "Disconnect")
            onClicked: {
                mainMenu.close()
                var id = folder.remote
                win.go(Fs.home)
                Remotes.disconnect(id)
                toast.show(Style.t("Scollegato", "Disconnected"))
            }
        }
        MenuRow {
            depth: 1
            icon: "add"
            visible: remoteOptions.open
            text: Style.t("Aggiungi una connessione…", "Add a connection…")
            onClicked: { mainMenu.close(); wizard.openNew() }
        }
        // le sincronizzazioni (0.9.5) e la «Sync manuale» (richiesta dell'utente del 15/11)
        MenuRow {
            depth: 1
            icon: "sync"
            visible: remoteOptions.open
            text: Style.t("Sincronizzazioni…", "Syncs…")
            detail: Syncs.list.length > 0 ? String(Syncs.list.length) : ""
            onClicked: { mainMenu.close(); syncSheet.open() }
        }
        MenuRow {
            depth: 1
            icon: "refresh"
            visible: remoteOptions.open && Syncs.list.length > 0
            text: Style.t("Sincronizza ora", "Sync now")
            onClicked: { mainMenu.close(); syncer.runAll(false, true) }
        }
        MenuRow {
            depth: 1
            icon: "lock"
            visible: remoteOptions.open && Remotes.vaultUnlocked
            text: Style.t("Chiudi la cassaforte delle password", "Lock the password vault")
            onClicked: { mainMenu.close(); Remotes.vaultLock(); toast.show(Style.t("Cassaforte chiusa", "Vault locked")) }
        }
        MenuSection { text: Style.t("Sistema", "System") }
        MenuRow {
            icon: "root"
            text: Fs.rootStarting ? Style.t("Modalità root: avvio…", "Root mode: starting…") : Style.t("Modalità root", "Root mode")
            checkable: true
            checked: Fs.rootActive
            onClicked: {
                mainMenu.close()
                if (Fs.rootActive) win.rootOff()
                else rootDialog.ask()
            }
        }
        MenuSection { text: Style.t("Tema", "Theme") }
        MenuRow { text: "RooTundra"; checkable: true; checked: settings.theme === "rootundra"; onClicked: settings.theme = "rootundra" }
        MenuRow { text: Style.t("SailfishOS classico", "Classic SailfishOS"); checkable: true; checked: settings.theme === "sailfish"; onClicked: settings.theme = "sailfish" }
        MenuSection { text: Style.t("Cartella", "Folder") }
        // le azioni sulla cartella in una tendina (richiesta dell'utente del 15/11)
        MenuRow {
            id: folderOptions
            property bool open: false
            icon: "folder"
            text: Style.t("Opzioni cartella", "Folder options")
            detail: open ? "" : (folder.path.split("/").pop() || "/")
            expandable: true
            expanded: open
            onClicked: open = !open
            onToggled: open = !open
            Connections {
                target: mainMenu
                function onAboutToShow() { folderOptions.open = false }
            }
        }
        MenuRow {
            depth: 1
            visible: folderOptions.open
            icon: "newfolder"
            text: Style.t("Nuova cartella", "New folder")
            enabled: Fs.isWritable(folder.path)
            onClicked: {
                mainMenu.close()
                nameDialog.ask(Style.t("Nuova cartella", "New folder"), Style.t("Nuova cartella", "New folder"), function (name) {
                    return Fs.makeDir(folder.path, name)
                })
            }
        }
        // «Nuovo file» subito sotto «Nuova cartella» (richiesta dell'utente del 12/11)
        MenuRow {
            depth: 1
            visible: folderOptions.open
            icon: "newfile"
            text: Style.t("Nuovo file", "New file")
            enabled: Fs.isWritable(folder.path)
            onClicked: {
                mainMenu.close()
                nameDialog.ask(Style.t("Nuovo file", "New file"), Style.t("Nuovo file.txt", "New file.txt"), function (name) {
                    return Fs.makeFile(folder.path, name)
                })
            }
        }
        // il cestino della memoria (o della connessione) dove si è, prima di
        // «Seleziona tutto» (richiesta dell'utente del 12/11)
        MenuRow {
            depth: 1
            visible: folderOptions.open
            icon: "trash"
            text: Style.t("Cestino", "Trash")
            detail: win.placeName(win.place)
            onClicked: {
                mainMenu.close()
                if (folder.remote !== "") trashSheet.openFor(Remotes.get(folder.remote))
                else trashSheet.openLocal(folder.path, win.placeName(win.place))
            }
        }
        MenuRow { depth: 1; visible: folderOptions.open; icon: "select"; text: Style.t("Seleziona tutto", "Select all"); enabled: folder.count > 0; onClicked: { mainMenu.close(); folder.selectAll() } }
        MenuRow { depth: 1; visible: folderOptions.open; icon: "info"; text: Style.t("Proprietà di questa cartella", "Folder properties"); onClicked: { mainMenu.close(); properties.openFor([folder.path]) } }
        MenuRow { depth: 1; visible: folderOptions.open; icon: "refresh"; text: Style.t("Aggiorna", "Refresh"); onClicked: { mainMenu.close(); folder.refresh() } }
        MenuRow {
            depth: 1
            visible: folderOptions.open && folder.remote === "" && Fs.isWritable(folder.path)
            icon: "sync"
            text: Style.t("Sincronizza questa cartella…", "Sync this folder…")
            onClicked: { mainMenu.close(); syncWizard.openNew(folder.path) }
        }
        Text {
            font.family: Style.fontFamily
            x: Style.margin
            height: 30 * Style.dp
            verticalAlignment: Text.AlignVCenter
            text: "RooTree · rootree-core " + folder.coreVersion()
            color: Style.dim
            font.pixelSize: 10 * Style.dp
        }
    }

    JobsSheet { id: jobsSheet }

    // le altre azioni sulla selezione, come il menù contestuale di Dolphin
    Sheet {
        id: moreMenu
        readonly property var one: folder.selectedCount === 1 ? Fs.info(folder.selectedPaths()[0]) : ({})
        MenuRow {
            icon: "openwith"
            text: Style.t("Apri con…", "Open with…")
            enabled: folder.selectedCount === 1 && !moreMenu.one.isDir
            onClicked: { moreMenu.close(); openWith.openFor(folder.selectedPaths()[0]) }
        }
        MenuRow {
            icon: "rename"
            text: Style.t("Rinomina", "Rename")
            enabled: folder.selectedCount === 1 && !folder.readOnly
            onClicked: {
                moreMenu.close()
                var p = folder.selectedPaths()[0]
                nameDialog.ask(Style.t("Rinomina", "Rename"), p.split("/").pop(), function (name) {
                    var err = Fs.rename(p, name)
                    if (!err) folder.clearSelection()
                    return err
                })
            }
        }
        MenuRow {
            icon: "extract"
            text: Style.t("Estrai qui", "Extract here")
            visible: !folder.readOnly && folder.remote === "" && folder.selectedCount === 1 && !moreMenu.one.isDir && Fs.isArchive(moreMenu.one.name || "")
            onClicked: { moreMenu.close(); Jobs.extract(folder.selectedPaths()[0], folder.path); folder.clearSelection() }
        }
        MenuRow {
            icon: "compress"
            text: Style.t("Comprimi…", "Compress…")
            visible: !folder.readOnly && folder.remote === ""
            enabled: Fs.isWritable(folder.path)
            onClicked: { moreMenu.close(); compressDialog.ask(folder.selectedPaths()) }
        }
        MenuRow { icon: "info"; text: Style.t("Proprietà", "Properties"); onClicked: { moreMenu.close(); properties.openFor(folder.selectedPaths()) } }
        MenuRow { icon: "selectall"; text: Style.t("Seleziona tutto", "Select all"); onClicked: { moreMenu.close(); folder.selectAll() } }
    }

    // Modalità root: la password di sviluppatore va a devel-su e non resta da
    // nessuna parte; se la modalità sviluppatore è spenta si spiega come accenderla
    AppDialog {
        id: rootDialog
        property string error
        readonly property bool ready: Fs.developerMode && Fs.rootHelperInstalled
        function ask() {
            error = ""
            rootPassword.text = ""
            open()
            if (ready) rootPassword.forceActiveFocus()
        }
        title: Style.t("Modalità root", "Root mode")
        message: !Fs.developerMode
            ? Style.t("Serve la modalità sviluppatore: Impostazioni → Strumenti per sviluppatori → Modalità sviluppatore, poi imposta lì una password. Torna qui e accendi la modalità root con quella password.",
                      "Developer mode is needed: Settings → Developer tools → Developer mode, then set a password there. Come back and turn on root mode with that password.")
            : !Fs.rootHelperInstalled
            ? Style.t("Manca l'aiutante root di RooTree (/usr/libexec/harbour-rootree/rootree-helper). Reinstalla RooTree e riprova.",
                      "RooTree's root helper (/usr/libexec/harbour-rootree/rootree-helper) is missing. Reinstall RooTree and try again.")
            : Style.t("RooTree potrà leggere e modificare tutto il sistema. L'app resta un'app normale: le operazioni da root le fa un aiutante separato, che si spegne con la modalità o chiudendo RooTree. Quando salvi un file da root, l'originale va in /home/debugging/harbour-rootree.",
                      "RooTree will be able to read and change the whole system. The app stays a normal app: root operations are done by a separate helper, which stops with the mode or when RooTree closes. When you save a file as root, the original goes to /home/debugging/harbour-rootree.")
        buttons: ready ? [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Accendi", "Turn on"), role: "accept" }]
                       : [{ text: "OK", role: "reject" }]
        onChosen: function (role) {
            if (role === "accept" && rootPassword.text !== "") {
                Fs.rootStart(rootPassword.text)
                rootPassword.text = ""
            }
        }
        TextField {
            id: rootPassword
            visible: rootDialog.ready
            font.family: Style.fontFamily
            width: parent.width
            color: Style.text
            font.pixelSize: Style.fontM
            echoMode: TextInput.Password
            placeholderText: Style.t("Password di sviluppatore", "Developer password")
            placeholderTextColor: Style.dim
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
            background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: rootPassword.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
            onAccepted: { rootDialog.close(); rootDialog.chosen("accept") }
        }
        Text { font.family: Style.fontFamily; visible: rootDialog.error !== ""; width: parent.width; wrapMode: Text.Wrap; text: rootDialog.error; color: Style.danger; font.pixelSize: Style.fontS }
    }

    // Comprimi: nome e formato; l'archivio nasce nella cartella corrente
    AppDialog {
        id: compressDialog
        property var paths: []
        property string format: "zip"
        readonly property var formats: ["zip", "tar.gz", "tar.xz", "tar.zst", "tar.bz2", "7z", "tar"]
        title: Style.t("Comprimi", "Compress")
        function ask(list) {
            paths = list
            var first = list[0].split("/").pop()
            // uno: il suo nome (senza estensione se è un file); più: il nome della cartella
            var base = list.length === 1 ? (Fs.info(list[0]).isDir || first.lastIndexOf(".") <= 0 ? first : first.substring(0, first.lastIndexOf(".")))
                                         : (folder.path.split("/").pop() || "archivio")
            archiveName.text = base
            open()
        }
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Comprimi", "Compress"), role: "accept" }]
        onChosen: function (role) {
            if (role !== "accept" || archiveName.text.trim() === "") return
            Jobs.compress(paths, Fs.join(folder.path, archiveName.text.trim() + "." + format))
            folder.clearSelection()
        }
        TextField {
            id: archiveName
            font.family: Style.fontFamily
            width: parent.width
            color: Style.text
            font.pixelSize: Style.fontM
            selectByMouse: true
            selectionColor: Style.accent
            background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: archiveName.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
        }
        Flow {
            width: parent.width
            spacing: 6 * Style.dp
            Repeater {
                model: compressDialog.formats
                delegate: SteelPill { text: "." + modelData; role: compressDialog.format === modelData ? "accept" : ""; onClicked: compressDialog.format = modelData }
            }
        }
        Text {
            font.family: Style.fontFamily
            width: parent.width
            wrapMode: Text.Wrap
            color: Style.dim
            font.pixelSize: Style.fontS
            text: ({ "zip": Style.t("Si apre ovunque (Windows, Android, Mac).", "Opens everywhere (Windows, Android, Mac)."),
                     "tar.gz": Style.t("Il classico di Linux; tiene permessi e link.", "The Linux classic; keeps permissions and links."),
                     "tar.xz": Style.t("Più piccolo, più lento da creare.", "Smaller, slower to create."),
                     "tar.zst": Style.t("Veloce e compatto.", "Fast and compact."),
                     "tar.bz2": Style.t("Vecchio standard di Linux.", "Old Linux standard."),
                     "7z": Style.t("Molto compatto; non salva i link.", "Very compact; does not store links."),
                     "tar": Style.t("Senza compressione.", "No compression.") })[compressDialog.format]
        }
    }

    SearchPanel {
        id: searchPanel
        onOpenRequested: function (e) {
            if (e.isDir) {
                close()
                win.go(e.path)
            } else if (Fs.isArchive(e.name)) {
                close()
                win.go(e.path)
            } else {
                win.openFile(e)
            }
        }
        onShowInFolder: function (p) {
            close()
            win.go(Fs.parentOf(p))
            var i = folder.indexOf(p.split("/").pop())
            if (i >= 0) folder.setSelected(i, true)
        }
    }

    OpenWithSheet { id: openWith }
    TrashSheet { id: trashSheet; store: settings; onRestored: folder.refresh() }
    ImageViewer {
        id: imageViewer
        onOpenWithRequested: function (p) { openWith.openFor(p) }
        onPropertiesRequested: function (p) { properties.openFor([p]) }
    }
    TextViewer {
        id: textViewer
        onOpenWithRequested: function (p) { openWith.openFor(p) }
        onMessage: function (text, isError) { toast.show(text, isError) }
        onSaved: folder.refresh()
    }
    PropertiesSheet { id: properties }

    // Nuova cartella / Rinomina: `apply(name)` rende "" se è andata, o l'errore
    AppDialog {
        id: nameDialog
        property var apply
        property string error
        function ask(title, initial, fn) {
            nameDialog.title = title
            apply = fn
            error = ""
            field.text = initial
            open()
            field.forceActiveFocus()
            // il nome senza l'estensione, come Dolphin
            var dot = initial.lastIndexOf(".")
            field.select(0, dot > 0 ? dot : initial.length)
        }
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: "OK", role: "accept" }]
        onChosen: function (role) {
            if (role !== "accept") return
            var err = apply(field.text.trim())
            if (err) {
                error = err
                open()
            } else {
                folder.refresh()
            }
        }
        // il campo con le maniglie sopra (sotto il testo c'è posto per i dischi)
        Item {
            width: parent.width
            height: field.height + 34 * Style.dp
            z: 2
            TextField {
                font.family: Style.fontFamily
                id: field
                width: parent.width
                color: Style.text
                font.pixelSize: Style.fontM
                selectByMouse: true
                // la selezione resta mentre si toccano i pulsanti qui sotto
                persistentSelection: true
                selectionColor: Style.accent
                background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: field.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
                onAccepted: { nameDialog.close(); nameDialog.chosen("accept") }
                // doppio tocco: la parola sotto il dito (poi le maniglie)
                TapHandler {
                    onDoubleTapped: function (ev) {
                        field.cursorPosition = field.positionAt(ev.position.x, ev.position.y)
                        field.selectWord()
                    }
                }
            }
            // le maniglie per scegliere col dito la parte da selezionare
            SelectionHandles { field: field }
        }
        // selezionare e copiare il nome o una parte (doppio tocco su una parola
        // o «Tutto», poi le maniglie), richiesta dell'utente dell'08/11
        Flow {
            width: parent.width
            spacing: 6 * Style.dp
            SteelPill { text: Style.t("Tutto", "All"); onClicked: { field.forceActiveFocus(); field.selectAll() } }
            SteelPill {
                text: Style.t("Senza estensione", "Without extension")
                visible: field.text.lastIndexOf(".") > 0
                onClicked: { field.forceActiveFocus(); field.select(0, field.text.lastIndexOf(".")) }
            }
            SteelPill {
                text: Style.t("Copia", "Copy")
                onClicked: {
                    if (field.selectedText === "") field.selectAll()
                    field.copy()
                    toast.show(Style.t("Copiato: ", "Copied: ") + field.selectedText)
                }
            }
            SteelPill { text: Style.t("Incolla", "Paste"); enabled: field.canPaste; onClicked: { field.forceActiveFocus(); field.paste() } }
        }
        Text { font.family: Style.fontFamily; visible: nameDialog.error !== ""; width: parent.width; wrapMode: Text.Wrap; text: nameDialog.error; color: Style.danger; font.pixelSize: Style.fontS }
    }

    AppDialog {
        id: confirm
        property var paths: []
        // "trash", "service:<tipo>" o "none:<perché>" (trash.rs)
        property string hint: "trash"
        readonly property bool canTrash: hint.indexOf("none:") !== 0
        // Dropbox: il definitivo alle app non lo concede
        readonly property bool canForever: hint !== "service:dropbox"
        readonly property bool toTrash: canTrash && !forever.checked
        readonly property string what: paths.length === 1 ? "«" + paths[0].split("/").pop() + "»" : Style.count(paths.length, Style.t("elemento", "item"), Style.t("elementi", "items"))
        title: toTrash ? Style.t("Spostare nel cestino?", "Move to trash?") : Style.t("Eliminare definitivamente?", "Delete permanently?")
        message: toTrash
            ? (hint === "service:dropbox"
               ? Style.t(what + " andrà nel cestino di Dropbox: si recupera per 30 giorni.", what + " will go to the Dropbox trash: recoverable for 30 days.")
               : Style.t(what + " andrà nel cestino: si potrà ripristinare dalla voce Cestino del menù.", what + " will go to the trash: it can be restored from Trash in the menu."))
            : (canTrash ? "" : Style.t("Qui il cestino non c'è (" + hint.substring(5) + "). ", "No trash here (" + hint.substring(5) + "). "))
              + Style.t(what + " non si potrà più recuperare.", what + " can no longer be recovered.")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" },
                  { text: toTrash ? Style.t("Nel cestino", "To trash") : Style.t("Elimina", "Delete"), role: "danger" }]
        CheckRow {
            id: forever
            visible: confirm.canTrash && confirm.canForever
            text: Style.t("Elimina definitivamente", "Delete permanently")
        }
        onChosen: function (role) {
            if (role === "danger") {
                if (toTrash) Jobs.trash(paths)
                else Jobs.remove(paths)
                folder.clearSelection()
            }
        }
    }

    // la domanda di un'operazione: il file esiste già, oppure un errore
    AppDialog {
        id: question
        property var q: ({})
        property var src: ({})
        property var dst: ({})
        readonly property bool conflict: q.state === 3
        // cartella su cartella: «unire?», come Dolphin (richiesta dell'utente, 08/11)
        readonly property bool dirConflict: conflict && q.askSrcIsDir === true && q.askDstIsDir === true
        title: dirConflict ? Style.t("La cartella esiste già", "Folder already exists")
             : conflict ? Style.t("Esiste già", "Already exists") : Style.t("Errore", "Error")
        message: dirConflict
            ? Style.t("In «" + (q.askDst || "").split("/").slice(-2, -1)[0] + "» c'è già una cartella «" + (q.askDst || "").split("/").pop() + "». Vuoi unire il contenuto? I file con lo stesso nome verranno chiesti uno per uno.",
                      "“" + (q.askDst || "").split("/").slice(-2, -1)[0] + "” already has a folder “" + (q.askDst || "").split("/").pop() + "”. Merge the contents? Files with the same name will be asked one by one.")
            : conflict
            ? Style.t("In «" + (q.askDst || "").split("/").slice(-2, -1)[0] + "» c'è già «" + (q.askDst || "").split("/").pop() + "».",
                      "“" + (q.askDst || "").split("/").pop() + "” already exists in “" + (q.askDst || "").split("/").slice(-2, -1)[0] + "”.")
            : "«" + (q.askSrc || "").split("/").pop() + "»: " + (q.error || "")
        buttons: dirConflict
            ? [{ text: Style.t("Unisci", "Merge"), role: "merge" }, { text: Style.t("Tieni entrambe", "Keep both"), role: "keep" },
               { text: Style.t("Salta", "Skip"), role: "skip" }, { text: Style.t("Annulla", "Cancel"), role: "cancel" }]
            : conflict
            ? (q.askDstIsDir ? [] : [{ text: Style.t("Sovrascrivi", "Overwrite"), role: "overwrite" }])
                .concat([{ text: Style.t("Tieni entrambi", "Keep both"), role: "keep" }, { text: Style.t("Salta", "Skip"), role: "skip" }, { text: Style.t("Annulla", "Cancel"), role: "cancel" }])
            : [{ text: Style.t("Riprova", "Retry"), role: "retry" }, { text: Style.t("Salta", "Skip"), role: "skip" }, { text: Style.t("Annulla", "Cancel"), role: "cancel" }]
        onChosen: function (role) {
            var a = { overwrite: 0, skip: 1, keep: 2, cancel: 3, retry: 4, merge: 5 }[role]
            Jobs.resolve(q.id, a, applyAll.checked)
        }
        Text {
            font.family: Style.fontFamily
            visible: question.conflict
            width: parent.width
            wrapMode: Text.Wrap
            color: Style.dim
            font.pixelSize: Style.fontS
            text: Style.t("Nuovo: ", "New: ") + (question.src.isDir ? Style.t("cartella", "folder") : Style.size(question.src.size || 0)) + ", " + Style.date(question.src.mtime) + "\n"
                  + Style.t("Esistente: ", "Existing: ") + (question.dst.isDir ? Style.t("cartella", "folder") : Style.size(question.dst.size || 0)) + ", " + Style.date(question.dst.mtime)
        }
        CheckRow {
            id: applyAll
            text: question.dirConflict ? Style.t("Fai così per tutte le cartelle", "Do this for all folders")
                : question.conflict ? Style.t("Fai così per tutti i file", "Do this for all files") : Style.t("Salta tutti gli errori", "Skip all errors")
        }
    }
    Connections {
        target: Jobs
        function onJobsChanged() {
            var q = Jobs.question
            if (q.id && !question.opened) {
                question.q = q
                question.src = q.state === 3 ? Fs.info(q.askSrc) : ({})
                question.dst = q.state === 3 ? Fs.info(q.askDst) : ({})
                applyAll.checked = false
                question.open()
            }
        }
        function onFinished(id, kind, state, error) {
            if (kind === 10 || kind === 11) {
                media.finished(id, kind, state, error)
                return
            }
            // dimensione e pagina «i» (per tipo, controllo): li segue chi li ha chiesti
            if (kind === 3 || kind >= 7)
                return
            if (win.remoteDone(id, state, error))
                return
            if (state === 6) {
                toast.show(Style.t("Operazione non riuscita: ", "Operation failed: ") + error, true)
            } else if (state === 5) {
                toast.show([Style.t("Copia completata", "Copy finished"), Style.t("Spostamento completato", "Move finished"),
                            Style.t("Eliminazione completata", "Deleted"), "",
                            Style.t("Estrazione completata", "Extraction finished"), Style.t("Archivio creato", "Archive created"),
                            Style.t("Spostato nel cestino", "Moved to trash")][kind])
                Jobs.forget(id)
            } else if (state === 7) {
                Jobs.forget(id)
            }
            folder.refresh()
            win.places = Fs.places()
        }
    }

    // ── sincronizzazioni (0.9.5) ─────────────────────────────────────────────
    // Solo con RooTree aperta (decisione dell'utente del 15/11): all'apertura,
    // quando un file della cartella del telefono cambia (Syncs.wanted), e a
    // mano. La connessione si apre come per un collegamento normale, ma senza
    // entrarci; la cassaforte, se serve, si chiede (una volta per giro).
    QtObject {
        id: syncer
        // connessioni in apertura per una sincronizzazione: id → [then(ok, msg, quiet)]
        property var waiting: ({})
        // le sincronizzazioni in fila: si aprono le connessioni una alla volta,
        // così la cassaforte si chiede una volta sola
        property var queue: []
        property bool busy: false
        // la cassaforte rifiutata: i giri automatici non la richiedono più fino alla chiusura
        property bool vaultDeclined: false

        function needsSecret(p) { return p.auth === "password" || p.auth === "token" || p.auth === "oauth" }
        // la connessione aperta, poi then(ok, messaggio, zitto)
        function ensure(p, interactive, then, why) {
            if (!p || !p.id) { then(false, Style.t("la connessione non c'è più", "the connection no longer exists"), false); return }
            if (Remotes.isConnected(p.id)) { then(true, "", false); return }
            if (needsSecret(p) && !Remotes.hasSecret(p.id)) {
                if (p.remember !== false && Remotes.vaultExists && !Remotes.vaultUnlocked) {
                    if (!interactive && vaultDeclined) { then(false, Style.t("cassaforte chiusa", "vault locked"), true); return }
                    vaultDialog.ask("unlock", function () { syncer.ensure(p, interactive, then, why) },
                                    function () { if (!interactive) syncer.vaultDeclined = true; then(false, Style.t("cassaforte chiusa", "vault locked"), !interactive) },
                                    why || "")
                    return
                }
                then(false, Style.t("manca la password: collegati una volta a «" + p.name + "» dal menù delle memorie e salvala nella cassaforte",
                                    "the password is missing: connect once to “" + p.name + "” from the storage menu and save it in the vault"), false)
                return
            }
            var w = Object.assign({}, waiting)
            var list = (w[p.id] || []).slice()
            list.push(then)
            w[p.id] = list
            waiting = w
            if (!Remotes.connecting(p.id)) Remotes.connect(p.id)
        }
        // dal collegamento: true se l'aveva chiesto una sincronizzazione
        function connectDone(id, code, message) {
            var list = waiting[id]
            if (!list) return false
            var w = Object.assign({}, waiting)
            delete w[id]
            waiting = w
            var msg = code === "NET" ? Style.t("non raggiungibile", "unreachable")
                    : code === "AUTH" || code === "LOGIN" ? Style.t("accesso rifiutato", "access refused")
                    : code === "MFA" ? Style.t("Mega chiede il codice: collegati una volta dal menù delle memorie", "Mega asks for the code: connect once from the storage menu")
                    : code.indexOf("HOSTKEY") === 0 ? Style.t("l'impronta del server è nuova o cambiata: collegati dal menù delle memorie", "the server fingerprint is new or changed: connect from the storage menu")
                    : (message || code)
            for (var i = 0; i < list.length; ++i) list[i](code === "", msg, false)
            return true
        }
        function fail(s, msg, quiet) {
            Syncs.setResult(s.id, Style.t("non riuscita: ", "failed: ") + msg, true)
            // l'avviso volatile (5 s) chiesto dall'utente; si riprova al prossimo giro
            if (!quiet) toast.show(Style.t("Impossibile sincronizzare «" + s.name + "»: ", "Cannot sync “" + s.name + "”: ") + msg, true)
        }
        function run(id, confirmed, interactive) {
            queue = queue.concat([{ id: id, confirmed: confirmed, interactive: interactive }])
            pump()
        }
        function runAll(manual, interactive) {
            var l = Syncs.list
            var n = 0
            for (var i = 0; i < l.length; ++i)
                if (manual || l[i].enabled !== false) { run(l[i].id, false, interactive); ++n }
            if (manual && n > 0) toast.show(Style.t("Sincronizzazione avviata", "Sync started"))
        }
        function pump() {
            if (busy || queue.length === 0) return
            var item = queue[0]
            queue = queue.slice(1)
            var s = Syncs.get(item.id)
            if (!s.id || Syncs.running(s.id)) { pump(); return }
            busy = true
            ensure(Remotes.get(s.remote), item.interactive, function (ok, msg, quiet) {
                if (ok) Syncs.start(s.id, item.confirmed)
                else syncer.fail(s, msg, quiet)
                syncer.busy = false
                syncer.pump()
            }, Style.t("Per sincronizzare «" + s.name + "» serve la cassaforte delle password.", "To sync “" + s.name + "” the password vault is needed."))
        }
        function finished(jobId, state, error) {
            var info = (Jobs.status(jobId).info) || ({})
            Jobs.forget(jobId)
            var s = Syncs.get(info.syncId || "")
            if (!s.id) return
            if (state === 6) { fail(s, error, false); return }
            if (state === 7) { Syncs.setResult(s.id, Style.t("annullata", "cancelled"), false); return }
            if (info.confirm) {
                Syncs.setResult(s.id, Style.t("aspetta la tua conferma", "waiting for your confirmation"), true)
                syncConfirm.ask(s, info)
                return
            }
            var parts = []
            if (info.uploaded) parts.push(Style.count(info.uploaded, Style.t("caricato", "uploaded"), Style.t("caricati", "uploaded")))
            if (info.downloaded) parts.push(Style.count(info.downloaded, Style.t("scaricato", "downloaded"), Style.t("scaricati", "downloaded")))
            if (info.deletedLocal) parts.push(Style.count(info.deletedLocal, Style.t("tolto dal telefono", "removed from the phone"), Style.t("tolti dal telefono", "removed from the phone")))
            if (info.deletedRemote) parts.push(Style.count(info.deletedRemote, Style.t("tolto dal remoto", "removed from the remote"), Style.t("tolti dal remoto", "removed from the remote")))
            if (info.conflicts) parts.push(Style.count(info.conflicts, Style.t("conflitto", "conflict"), Style.t("conflitti", "conflicts")))
            var skipped = info.skipped ? info.skipped.length : 0
            if (skipped) parts.push(Style.count(skipped, Style.t("non riuscito", "failed"), Style.t("non riusciti", "failed")))
            var result = parts.length > 0 ? parts.join(", ") : Style.t("già allineata", "already in sync")
            Syncs.setResult(s.id, result + (skipped ? " (" + info.skipped[0] + ")" : ""), skipped > 0)
            if (parts.length > 0) toast.show("«" + s.name + "»: " + result, skipped > 0)
            if (info.downloaded || info.deletedLocal || info.conflicts) {
                folder.refresh()
                win.places = Fs.places()
            }
        }
    }
    Connections {
        target: Jobs
        function onFinished(id, kind, state, error) { if (kind === 9) syncer.finished(id, state, error) }
    }
    Connections {
        target: Syncs
        function onWanted(id) { syncer.run(id, false, false) }
    }
    // all'apertura, dopo che la finestra si è vista
    Timer {
        id: openingSync
        interval: 2500
        onTriggered: if (Syncs.list.length > 0) syncer.runAll(false, false)
    }
    // le salvaguardie: troppe eliminazioni, o una parte vuota tutta insieme
    AppDialog {
        id: syncConfirm
        property var sync: ({})
        property var info: ({})
        function ask(s, i) { sync = s; info = i; open() }
        readonly property string connName: (Remotes.get(sync.remote || "").name || Style.t("remoto", "remote"))
        title: Style.t("«" + (sync.name || "") + "»: confermi?", "“" + (sync.name || "") + "”: confirm?")
        message: info.confirm === "empty-remote"
            ? Style.t("La cartella «" + (sync.remotePath || "/") + "» su «" + connName + "» adesso è vuota, mentre l'ultima volta aveva dei file. Se li hai spostati tu (per archiviarli), conferma: " + Style.count(info.pending || 0, "file sparirà", "file spariranno") + " dal telefono, nel cestino di RooTree. Se invece il NAS è montato male o la cartella è sbagliata, annulla: non si tocca niente.",
                      "The folder “" + (sync.remotePath || "/") + "” on “" + connName + "” is now empty, while last time it had files. If you moved them yourself (to archive them), confirm: " + (info.pending || 0) + " files will disappear from the phone, into RooTree's trash. If instead the NAS is badly mounted or the folder is wrong, cancel: nothing is touched.")
            : info.confirm === "empty-local"
            ? Style.t("La cartella del telefono «" + (sync.local || "").split("/").pop() + "» adesso è vuota, mentre l'ultima volta aveva dei file. Confermando, " + Style.count(info.pending || 0, "file verrà tolto", "file verranno tolti") + " anche da «" + connName + "» (nel suo cestino). Se la scheda SD è stata tolta o la cartella spostata, annulla.",
                      "The phone folder “" + (sync.local || "").split("/").pop() + "” is now empty, while last time it had files. Confirming, " + (info.pending || 0) + " files will also be removed from “" + connName + "” (into its trash). If the SD card was removed or the folder moved, cancel.")
            : Style.t("Questo giro eliminerebbe " + (info.pending || 0) + " file (più di 50). Finiscono nel cestino e si possono recuperare. Confermi?",
                      "This run would delete " + (info.pending || 0) + " files (more than 50). They go to the trash and can be recovered. Confirm?")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Conferma", "Confirm"), role: "danger" }]
        onChosen: function (role) {
            if (role === "danger") syncer.run(sync.id, true, true)
            else Syncs.setResult(sync.id, Style.t("eliminazioni non confermate: niente fatto", "deletions not confirmed: nothing done"), true)
        }
    }
    SyncWizard {
        id: syncWizard
        onSaved: function (id) {
            toast.show(Style.t("Sincronizzazione salvata: primo giro…", "Sync saved: first run…"))
            syncer.run(id, false, true)
        }
        onConnectRequested: function (remoteId, then) {
            syncer.ensure(Remotes.get(remoteId), true, function (ok, msg) {
                if (ok) then()
                else toast.show(Style.t("Collegamento non riuscito: ", "Connection failed: ") + msg, true)
            })
        }
    }
    SyncSheet {
        id: syncSheet
        onNewRequested: syncWizard.openNew(folder.remote === "" ? folder.path : "")
        onRunRequested: function (id) { syncer.run(id, false, true) }
        onRunAllRequested: syncer.runAll(true, true)
        onEditRequested: function (s) { syncWizard.openFor(s) }
        onDeleteRequested: function (s) { deleteSync.sync = s; deleteSync.open() }
    }
    AppDialog {
        id: deleteSync
        property var sync: ({})
        title: Style.t("Eliminare la sincronizzazione?", "Delete the sync?")
        message: Style.t("«" + (sync.name || "") + "» non sincronizzerà più. I file, sul telefono e sul remoto, restano dove sono.",
                         "“" + (sync.name || "") + "” will stop syncing. The files, on the phone and on the remote, stay where they are.")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Elimina", "Delete"), role: "danger" }]
        onChosen: function (role) { if (role === "danger") Syncs.remove(sync.id) }
    }

    // sopra i pannelli a tutto schermo (editor, procedure) e sopra la tastiera:
    // nella finestra principale l'esito di un salvataggio dall'editor non si vedeva
    Toast {
        id: toast
        parent: Overlay.overlay
        z: 1000
        y: parent ? parent.height - Math.max(bottomBar.height, Style.keyboardHeight(parent.height, parent.width)) - height - 40 * Style.dp : 0
    }
}
