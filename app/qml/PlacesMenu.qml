import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Il menù a tendina delle memorie (al posto del pannello «Risorse» di
// Dolphin): Memoria interna, Memoria Android, Altre memorie (scheda SD, USB,
// file system), Rete e cloud (le connessioni remote salvate), poi i segnalibri.
// Ogni memoria si apre con la sua freccia sulle cartelle predefinite che ha
// (Documents, Downloads, Pictures, Music, Videos).
Popup {
    id: menu
    property string currentPath
    property var places: []
    property bool otherOpen: false
    // le memorie aperte sulle loro cartelle, per percorso
    property var openPlaces: ({})
    signal navigate(string path)
    // le connessioni remote (Fase 5): aggiungerne una, aprirla, il suo menù
    signal addConnection()
    signal openConnection(var profile)
    signal connectionActions(var profile)
    // la «i» di una memoria: la pagina con spazio, file system e stato
    signal placeInfo(var place)

    parent: Overlay.overlay
    modal: true
    dim: true
    padding: 0
    x: 6 * Style.dp
    width: parent ? parent.width - 12 * Style.dp : 0
    height: Math.min(parent ? parent.height * 0.8 : 0, col.implicitHeight + 2 * ice.frame + 8 * Style.dp)
    Overlay.modal: Rectangle { color: "#99000000" }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120 } }
    // una lastra di ghiaccio, come il cassetto delle app di RooTundra
    background: IcePanel { id: ice }

    onAboutToShow: {
        places = Fs.places()
        // si apre sempre tutto chiuso: le memorie e i segnalibri, il resto a richiesta
        otherOpen = false
        openPlaces = ({})
    }

    function go(path) { close(); navigate(path) }
    function info(p) { close(); placeInfo(p) }
    function freeText(p) { return p.total > 0 ? Style.freeOf(p.free, p.total) : "" }
    // le cartelle predefinite che ci sono davvero; su Android e sulle schede
    // si chiamano anche Download e Movies
    readonly property var standardDirs: [
        { names: ["Documents"], image: "tipi/folder-documents.png" },
        { names: ["Downloads", "Download"], image: "tipi/folder-download.png" },
        { names: ["Pictures"], image: "tipi/folder-pictures.png" },
        { names: ["Music"], image: "tipi/folder-music.png" },
        { names: ["Videos", "Movies"], image: "tipi/folder-videos.png" }
    ]
    function dirsOf(base) {
        var out = []
        if (!base || base === "/")
            return out
        for (var i = 0; i < standardDirs.length; ++i) {
            var d = standardDirs[i]
            for (var j = 0; j < d.names.length; ++j) {
                var path = Fs.join(base, d.names[j])
                if (Fs.exists(path)) { out.push({ name: d.names[j], path: path, image: d.image }); break }
            }
        }
        return out
    }
    function isOpen(path) { return openPlaces[path] === true }
    function toggle(path) { var o = Object.assign({}, openPlaces); o[path] = !o[path]; openPlaces = o }

    function placeOf(kind) { for (var i = 0; i < places.length; ++i) if (places[i].kind === kind) return places[i]; return null }

    contentItem: Flickable {
        anchors.fill: parent
        anchors.margins: ice.frame + 4 * Style.dp
        contentHeight: col.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Column {
            id: col
            width: parent.width

            MenuSection { text: Style.t("Memorie", "Storage") }
            PlaceRow {
                owner: menu
                place: menu.placeOf(0)
                icon: "internal"
                text: Style.t("Memoria interna", "Internal storage")
            }
            PlaceRow {
                owner: menu
                place: menu.placeOf(1)
                icon: "android"
                text: Style.t("Memoria Android", "Android storage")
            }
            MenuRow {
                icon: "disk"
                text: Style.t("Altre memorie", "Other storage")
                expandable: true
                expanded: menu.otherOpen
                onClicked: menu.otherOpen = !menu.otherOpen
                onToggled: menu.otherOpen = !menu.otherOpen
            }
            Column {
                width: parent.width
                visible: menu.otherOpen
                Repeater {
                    model: menu.places.filter(function (p) { return p.kind === 2 || p.kind === 3 })
                    delegate: PlaceRow {
                        owner: menu
                        depth: 1
                        place: modelData
                        icon: modelData.kind === 2 ? "sdcard" : "usb"
                        // il codice della scheda (UUID) non si mostra qui: è nella pagina della «i»
                        text: modelData.kind === 2 ? Style.t("Scheda SD", "SD card") : "USB" + (modelData.label ? " · " + modelData.label : "")
                    }
                }
                MenuRow {
                    readonly property var p: menu.placeOf(4)
                    indent: true
                    icon: "root"
                    text: Style.t("File system (/)", "File system (/)")
                    detail: p ? menu.freeText(p) : ""
                    reserveToggle: true
                    infoButton: p !== null
                    onInfoClicked: menu.info(p)
                    onClicked: menu.go("/")
                }
            }

            MenuSection { text: Style.t("Rete e cloud", "Network and cloud") }
            Repeater {
                model: Remotes.list
                delegate: Item {
                    width: col.width
                    height: Style.row
                    MenuRow {
                        anchors.left: parent.left
                        anchors.right: more.left
                        width: undefined
                        icon: Remotes.typeInfo(modelData.type).icon || "cloud"
                        text: modelData.name
                        detail: ""
                        onClicked: { menu.close(); menu.openConnection(modelData) }
                    }
                    Text {
                        font.family: Style.fontFamily
                        anchors { right: more.left; rightMargin: 2 * Style.dp; verticalCenter: parent.verticalCenter }
                        width: Math.min(implicitWidth, parent.width * 0.42)
                        text: Remotes.describe(modelData)
                        // già scritto nel nome (Mega · email, Dropbox): due volte si accavallano
                        visible: modelData.name.toLowerCase().indexOf(text.toLowerCase()) < 0
                        color: Style.dim
                        font.pixelSize: Style.fontS
                        elide: Text.ElideMiddle
                    }
                    IconButton {
                        id: more
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        icon: "more"
                        onClicked: { menu.close(); menu.connectionActions(modelData) }
                    }
                }
            }
            MenuRow {
                icon: "add"
                text: Style.t("Aggiungi una connessione…", "Add a connection…")
                detail: Remotes.list.length === 0 ? "SFTP, WebDAV, SMB, Dropbox, Mega…" : ""
                onClicked: { menu.close(); menu.addConnection() }
            }

            MenuSection { text: Style.t("Segnalibri", "Bookmarks") }
            Repeater {
                model: Fs.bookmarks
                delegate: Item {
                    width: col.width
                    height: Style.row
                    MenuRow {
                        anchors.left: parent.left
                        anchors.right: remove.left
                        width: undefined
                        icon: "bookmark"
                        text: modelData.name
                        detail: modelData.exists ? "" : Style.t("non c'è", "missing")
                        enabled: modelData.exists
                        onClicked: menu.go(modelData.path)
                    }
                    IconButton {
                        id: remove
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        icon: "close"
                        onClicked: Fs.removeBookmark(modelData.path)
                    }
                }
            }
            MenuRow {
                readonly property bool marked: Fs.bookmarks.some(function (b) { return b.path === menu.currentPath })
                icon: "bookmark"
                text: marked ? Style.t("Togli questa cartella dai segnalibri", "Remove this folder from bookmarks")
                             : Style.t("Aggiungi questa cartella ai segnalibri", "Bookmark this folder")
                onClicked: { if (marked) Fs.removeBookmark(menu.currentPath); else Fs.addBookmark(menu.currentPath) }
            }
        }
    }
}
