import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Scegliere un file navigando fra le cartelle, a tutto schermo: tocco su una
// cartella = entra, su un file = scelto. File nascosti visibili di base (le
// chiavi SSH stanno in ~/.ssh). Con `markKeys` i file che sono chiavi private
// hanno l'etichetta «chiave privata».
Popup {
    id: picker
    property string title: Style.t("Scegli un file", "Choose a file")
    property bool markKeys: false
    // scegliere una cartella (le sincronizzazioni): i file si vedono ma non si
    // toccano, in basso «Usa questa cartella»
    property bool dirsOnly: false
    signal picked(string path)

    parent: Overlay.overlay
    x: 0; y: 0
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    padding: 0
    modal: true
    closePolicy: Popup.NoAutoClose
    background: Rectangle { color: Style.bg }

    function openAt(dir) {
        fm.setSorting(0, false, true) // per nome, cartelle prima
        fm.showHidden = !dirsOnly
        fm.path = Fs.exists(dir) ? dir : Fs.home
        open()
    }

    FolderModel { id: fm }

    contentItem: Item {
        SteelPanel {
            id: top
            anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: -border.width; rightMargin: -border.width; topMargin: -border.width }
            height: Style.bar + pathText.height + 8 * Style.dp
            ledWidth: width
            IconButton { id: closeBtn; anchors { left: parent.left; top: parent.top; topMargin: 2 * Style.dp } icon: "close"; onClicked: picker.close() }
            Text {
                font.family: Style.fontFamily
                anchors { left: closeBtn.right; right: upBtn.left; verticalCenter: closeBtn.verticalCenter }
                text: picker.title
                color: Style.text
                font.pixelSize: Style.fontL
                font.bold: true
                elide: Text.ElideRight
            }
            IconButton { id: hiddenBtn; anchors { right: parent.right; verticalCenter: closeBtn.verticalCenter } icon: fm.showHidden ? "shown" : "hidden"; checked: fm.showHidden; onClicked: fm.showHidden = !fm.showHidden }
            IconButton { id: upBtn; anchors { right: hiddenBtn.left; verticalCenter: closeBtn.verticalCenter } icon: "up"; enabled: fm.path !== "/" && !/^\/@remote\/[^\/]+$/.test(fm.path); onClicked: fm.path = Fs.parentOf(fm.path) }
            Text {
                id: pathText
                font.family: Style.fontFamily
                anchors { left: parent.left; right: parent.right; top: closeBtn.bottom; leftMargin: Style.margin; rightMargin: Style.margin }
                // su un server il percorso suo, non quello interno /@remote/<id>/…
                text: fm.path.indexOf("/@remote/") === 0 ? (fm.path.replace(/^\/@remote\/[^\/]+/, "") || "/")
                      : fm.path.indexOf(Fs.home) === 0 ? "~" + fm.path.substring(Fs.home.length) : fm.path
                color: Style.dim
                font.pixelSize: Style.fontS
                elide: Text.ElideMiddle
            }
        }
        ListView {
            id: list
            anchors { left: parent.left; right: parent.right; top: top.bottom; bottom: picker.dirsOnly ? useBar.top : parent.bottom }
            clip: true
            model: fm
            boundsBehavior: Flickable.StopAtBounds
            delegate: Rectangle {
                width: list.width
                height: Style.row
                color: area.pressed ? Style.pressed : "transparent"
                readonly property bool key: picker.markKeys && !isDir && Fs.isPrivateKey(path)
                opacity: picker.dirsOnly && !isDir ? 0.4 : 1
                FileIcon {
                    id: icon
                    anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                    width: 40 * Style.dp
                    height: width
                    small: true
                    path: model.path
                    category: model.category
                    thumbnails: false
                    opacity: hidden ? 0.6 : 1
                }
                Column {
                    anchors { left: icon.right; leftMargin: Style.margin; right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                    Text { font.family: Style.fontFamily; width: parent.width; text: name; color: Style.text; font.pixelSize: Style.fontM; elide: Text.ElideMiddle }
                    Text {
                        font.family: Style.fontFamily
                        width: parent.width
                        text: parent.parent.key ? Style.t("chiave privata", "private key") : (isDir ? "" : Style.size(size) + " · " + Style.date(mtime))
                        color: parent.parent.key ? Style.accent : Style.dim
                        font.pixelSize: Style.fontS
                        font.bold: parent.parent.key
                    }
                }
                Rectangle { anchors { left: icon.right; right: parent.right; bottom: parent.bottom } height: 1; color: Style.line; opacity: 0.6 }
                MouseArea {
                    id: area
                    anchors.fill: parent
                    enabled: isDir || !picker.dirsOnly
                    onClicked: {
                        if (isDir) fm.path = path
                        else { picker.picked(path); picker.close() }
                    }
                }
            }
            Text {
                font.family: Style.fontFamily
                anchors.centerIn: parent
                visible: list.count === 0
                text: fm.error ? fm.error : Style.t("Cartella vuota", "Empty folder")
                color: Style.dim
                font.pixelSize: Style.fontM
            }
        }
        SteelPanel {
            id: useBar
            visible: picker.dirsOnly
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: -border.width; rightMargin: -border.width; bottomMargin: -border.width }
            height: picker.dirsOnly ? Style.bar + 10 * Style.dp : 0
            led: ""
            SteelPill {
                anchors { right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                role: "accept"
                enabled: fm.error === ""
                text: Style.t("Usa questa cartella", "Use this folder")
                onClicked: { picker.picked(fm.path); picker.close() }
            }
        }
    }
}
