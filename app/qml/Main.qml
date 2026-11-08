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
        var best = null
        for (var i = 0; i < places.length; ++i) {
            var p = places[i]
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
        }
        return "/"
    }
    function placeIcon(p) { return ["internal", "android", "sdcard", "usb", "root"][p.kind] }

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
    function goUp() { if (folder.path !== "/") go(Fs.parentOf(folder.path)) }

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
        openFile(e)
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

    function deleteSelected() {
        var list = folder.selectedPaths()
        confirm.paths = list
        confirm.message = list.length === 1
            ? Style.t("«" + list[0].split("/").pop() + "» verrà eliminato definitivamente.", "“" + list[0].split("/").pop() + "” will be permanently deleted.")
            : Style.t(list.length + " elementi verranno eliminati definitivamente.", list.length + " items will be permanently deleted.")
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

    // ── la cartella ─────────────────────────────────────────────────────────
    Item {
        id: content
        anchors { left: parent.left; right: parent.right; top: header.bottom; bottom: status.top }
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
                    thumbnails: settings.thumbnails && folder.archive === ""
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
                    thumbnails: settings.thumbnails && folder.archive === ""
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
                      ? "\n" + Style.t("Per le cartelle di sistema servirà la modalità root.", "System folders will need root mode.") : "")
                color: Style.dim
                font.pixelSize: Style.fontS
            }
        }
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
                s += " · " + Style.t(Style.size(win.place.free) + " liberi", Style.size(win.place.free) + " free")
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
        readonly property var running: Jobs.jobs.filter(function (j) { return j.kind !== 3 && j.state < 5 })[0]
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
            IconButton { width: parent.w; height: parent.height; icon: "up"; enabled: folder.path !== "/"; onClicked: win.goUp() }
            IconButton { width: parent.w; height: parent.height; icon: "search"; enabled: folder.archive === ""; onClicked: searchPanel.openIn(folder.path) }
            IconButton { width: parent.w; height: parent.height; icon: settings.view === "grid" ? "list" : "grid"; onClicked: settings.view = settings.view === "grid" ? "list" : "grid" }
            IconButton {
                width: parent.w; height: parent.height
                icon: "jobs"
                opacity: Jobs.jobs.some(function (j) { return j.kind !== 3 }) ? 1 : 0.35
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
            IconButton { width: parent.w; height: parent.height; icon: "cut"; label: Style.t("Taglia", "Cut"); enabled: folder.archive === ""; onClicked: { Jobs.setClipboard(folder.selectedPaths(), true); folder.clearSelection() } }
            IconButton {
                width: parent.w; height: parent.height
                icon: "share"; label: Style.t("Condividi", "Share")
                // si condividono i file, non le cartelle
                enabled: folder.archive === "" && folder.selectedCount > 0 && folder.selectedPaths().every(function (p) { return !Fs.info(p).isDir })
                onClicked: { if (!Opener.share(folder.selectedPaths()) && Opener.lastError()) win.showSandboxError(); folder.clearSelection() }
            }
            IconButton { width: parent.w; height: parent.height; icon: "delete"; label: Style.t("Elimina", "Delete"); enabled: folder.archive === ""; onClicked: win.deleteSelected() }
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
    }

    Sheet {
        id: mainMenu
        MenuSection { text: Style.t("Vista", "View") }
        MenuRow { icon: "grid"; text: Style.t("Icone", "Icons"); checkable: true; checked: settings.view === "grid"; onClicked: { settings.view = "grid"; mainMenu.close() } }
        MenuRow { icon: "list"; text: Style.t("Dettagli", "Details"); checkable: true; checked: settings.view === "list"; onClicked: { settings.view = "list"; mainMenu.close() } }
        MenuRow { icon: folder.showHidden ? "shown" : "hidden"; text: Style.t("Mostra i file nascosti", "Show hidden files"); checkable: true; checked: folder.showHidden; onClicked: folder.showHidden = !folder.showHidden }
        MenuRow { icon: "image"; text: Style.t("Mostra le anteprime", "Show previews"); checkable: true; checked: settings.thumbnails; onClicked: settings.thumbnails = !settings.thumbnails }
        MenuSection { text: Style.t("Ordina per", "Sort by") }
        Repeater {
            model: [Style.t("Nome", "Name"), Style.t("Dimensione", "Size"), Style.t("Data di modifica", "Modified"), Style.t("Tipo", "Type")]
            delegate: MenuRow { text: modelData; checkable: true; checked: folder.sortKey === index; onClicked: win.changeSort(index, folder.descending, folder.dirsFirst) }
        }
        MenuRow {
            icon: "sort"
            // per data l'ordine normale è dal più recente: l'inverso lo dice
            text: folder.sortKey === 2 ? Style.t("Prima i più vecchi", "Oldest first") : Style.t("Ordine inverso", "Descending")
            checkable: true; checked: folder.descending
            onClicked: win.changeSort(folder.sortKey, !folder.descending, folder.dirsFirst)
        }
        MenuRow { icon: "folder"; text: Style.t("Cartelle prima", "Folders first"); checkable: true; checked: folder.dirsFirst; onClicked: win.changeSort(folder.sortKey, folder.descending, !folder.dirsFirst) }
        MenuRow {
            icon: "check"
            text: Style.t("Solo per questa cartella", "Only for this folder")
            checkable: true; checked: settings.sortPerFolder
            onClicked: win.setSortPerFolder(!settings.sortPerFolder)
        }
        MenuSection { text: Style.t("Tema", "Theme") }
        MenuRow { text: "RooTundra"; checkable: true; checked: settings.theme === "rootundra"; onClicked: settings.theme = "rootundra" }
        MenuRow { text: Style.t("SailfishOS classico", "Classic SailfishOS"); checkable: true; checked: settings.theme === "sailfish"; onClicked: settings.theme = "sailfish" }
        MenuSection { text: Style.t("Cartella", "Folder") }
        MenuRow {
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
        MenuRow { icon: "select"; text: Style.t("Seleziona tutto", "Select all"); enabled: folder.count > 0; onClicked: { mainMenu.close(); folder.selectAll() } }
        MenuRow { icon: "info"; text: Style.t("Proprietà di questa cartella", "Folder properties"); onClicked: { mainMenu.close(); properties.openFor([folder.path]) } }
        MenuRow { icon: "refresh"; text: Style.t("Aggiorna", "Refresh"); onClicked: { mainMenu.close(); folder.refresh() } }
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
            enabled: folder.selectedCount === 1 && folder.archive === ""
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
            visible: folder.archive === "" && folder.selectedCount === 1 && !moreMenu.one.isDir && Fs.isArchive(moreMenu.one.name || "")
            onClicked: { moreMenu.close(); Jobs.extract(folder.selectedPaths()[0], folder.path); folder.clearSelection() }
        }
        MenuRow {
            icon: "compress"
            text: Style.t("Comprimi…", "Compress…")
            visible: folder.archive === ""
            enabled: Fs.isWritable(folder.path)
            onClicked: { moreMenu.close(); compressDialog.ask(folder.selectedPaths()) }
        }
        MenuRow { icon: "info"; text: Style.t("Proprietà", "Properties"); onClicked: { moreMenu.close(); properties.openFor(folder.selectedPaths()) } }
        MenuRow { icon: "selectall"; text: Style.t("Seleziona tutto", "Select all"); onClicked: { moreMenu.close(); folder.selectAll() } }
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
    ImageViewer {
        id: imageViewer
        onOpenWithRequested: function (p) { openWith.openFor(p) }
        onPropertiesRequested: function (p) { properties.openFor([p]) }
    }
    TextViewer {
        id: textViewer
        onOpenWithRequested: function (p) { openWith.openFor(p) }
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
        }
        // selezionare e copiare il nome (o una parte: trascinando o col doppio
        // tocco su una parola), richiesta dell'utente dell'08/11
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
        title: Style.t("Eliminare?", "Delete?")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Elimina", "Delete"), role: "danger" }]
        onChosen: function (role) {
            if (role === "danger") {
                Jobs.remove(paths)
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
        title: conflict ? Style.t("Esiste già", "Already exists") : Style.t("Errore", "Error")
        message: conflict
            ? Style.t("In «" + (q.askDst || "").split("/").slice(-2, -1)[0] + "» c'è già «" + (q.askDst || "").split("/").pop() + "».",
                      "“" + (q.askDst || "").split("/").pop() + "” already exists in “" + (q.askDst || "").split("/").slice(-2, -1)[0] + "”.")
            : "«" + (q.askSrc || "").split("/").pop() + "»: " + (q.error || "")
        buttons: conflict
            ? (q.askDstIsDir ? [] : [{ text: Style.t("Sovrascrivi", "Overwrite"), role: "overwrite" }])
                .concat([{ text: Style.t("Tieni entrambi", "Keep both"), role: "keep" }, { text: Style.t("Salta", "Skip"), role: "skip" }, { text: Style.t("Annulla", "Cancel"), role: "cancel" }])
            : [{ text: Style.t("Riprova", "Retry"), role: "retry" }, { text: Style.t("Salta", "Skip"), role: "skip" }, { text: Style.t("Annulla", "Cancel"), role: "cancel" }]
        onChosen: function (role) {
            var a = { overwrite: 0, skip: 1, keep: 2, cancel: 3, retry: 4 }[role]
            Jobs.resolve(q.id, a, applyAll.checked)
        }
        Text {
            font.family: Style.fontFamily
            visible: question.conflict
            width: parent.width
            wrapMode: Text.Wrap
            color: Style.dim
            font.pixelSize: Style.fontS
            text: Style.t("Nuovo: ", "New: ") + Style.size(question.src.size || 0) + ", " + Style.date(question.src.mtime) + "\n"
                  + Style.t("Esistente: ", "Existing: ") + (question.dst.isDir ? Style.t("cartella", "folder") : Style.size(question.dst.size || 0)) + ", " + Style.date(question.dst.mtime)
        }
        CheckRow { id: applyAll; text: question.conflict ? Style.t("Fai così per tutti", "Do this for all") : Style.t("Salta tutti gli errori", "Skip all errors") }
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
            if (kind === 3)
                return
            if (state === 6) {
                toast.show(Style.t("Operazione non riuscita: ", "Operation failed: ") + error, true)
            } else if (state === 5) {
                toast.show([Style.t("Copia completata", "Copy finished"), Style.t("Spostamento completato", "Move finished"),
                            Style.t("Eliminazione completata", "Deleted"), "",
                            Style.t("Estrazione completata", "Extraction finished"), Style.t("Archivio creato", "Archive created")][kind])
                Jobs.forget(id)
            } else if (state === 7) {
                Jobs.forget(id)
            }
            folder.refresh()
            win.places = Fs.places()
        }
    }

    Toast { id: toast; y: parent.height - bottomBar.height - height - 40 * Style.dp; z: 10 }
}
