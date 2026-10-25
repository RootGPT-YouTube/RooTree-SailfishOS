import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Il menù a tendina delle memorie (al posto del pannello «Risorse» di
// Dolphin): Memoria interna, Memoria Android, Altre memorie (scheda SD, USB,
// file system e, più avanti, cloud e server remoti), poi i segnalibri.
Popup {
    id: menu
    property string currentPath
    property var places: []
    property bool otherOpen: false
    signal navigate(string path)

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
        // con una scheda SD o una chiavetta inserita, «Altre memorie» si apre da sé
        otherOpen = places.some(function (p) { return p.kind === 2 || p.kind === 3 })
    }

    function go(path) { close(); navigate(path) }
    function freeText(p) { return p.total > 0 ? Style.t(Style.size(p.free) + " liberi", Style.size(p.free) + " free") : "" }
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
            MenuRow {
                readonly property var p: menu.placeOf(0)
                visible: p !== null
                icon: "internal"
                text: Style.t("Memoria interna", "Internal storage")
                detail: p ? menu.freeText(p) : ""
                onClicked: menu.go(p.path)
            }
            MenuRow {
                readonly property var p: menu.placeOf(1)
                visible: p !== null
                icon: "android"
                text: Style.t("Memoria Android", "Android storage")
                detail: p ? menu.freeText(p) : ""
                onClicked: menu.go(p.path)
            }
            MenuRow {
                icon: "disk"
                text: Style.t("Altre memorie", "Other storage")
                detail: menu.otherOpen ? "▴" : "▾"
                onClicked: menu.otherOpen = !menu.otherOpen
            }
            Column {
                width: parent.width
                visible: menu.otherOpen
                Repeater {
                    model: menu.places.filter(function (p) { return p.kind === 2 || p.kind === 3 })
                    delegate: MenuRow {
                        indent: true
                        icon: modelData.kind === 2 ? "sdcard" : "usb"
                        text: (modelData.kind === 2 ? Style.t("Scheda SD", "SD card") : "USB") + (modelData.label ? " · " + modelData.label : "")
                        detail: menu.freeText(modelData)
                        onClicked: menu.go(modelData.path)
                    }
                }
                MenuRow {
                    readonly property var p: menu.placeOf(4)
                    indent: true
                    icon: "root"
                    text: Style.t("File system (/)", "File system (/)")
                    detail: p ? menu.freeText(p) : ""
                    onClicked: menu.go("/")
                }
                // remoti: arrivano con la fase 5
                MenuRow { indent: true; enabled: false; icon: "cloud"; text: "Cloud · Google Drive, OneDrive, Mega…"; detail: Style.t("in arrivo", "coming") }
                MenuRow { indent: true; enabled: false; icon: "ssh"; text: "SSH / SFTP"; detail: Style.t("in arrivo", "coming") }
                MenuRow { indent: true; enabled: false; icon: "server"; text: "Nextcloud / WebDAV"; detail: Style.t("in arrivo", "coming") }
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
