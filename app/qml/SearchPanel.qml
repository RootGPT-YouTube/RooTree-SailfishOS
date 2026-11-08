import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// La ricerca, a tutto schermo: si cerca dalla cartella in cui si è, in giù.
// Per nome (anche con * e ?) o nel contenuto dei file; le opzioni per le
// espressioni regolari, le maiuscole e i file nascosti. I risultati arrivano
// mentre la ricerca va avanti. Tocco: apre il file (o entra nella cartella);
// il disco a destra porta alla cartella che lo contiene.
Popup {
    id: panel
    property string root
    signal openRequested(var entry)
    signal showInFolder(string path)

    parent: Overlay.overlay
    x: 0; y: 0
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    padding: 0
    modal: true
    closePolicy: Popup.NoAutoClose
    background: Rectangle {
        color: Style.bg
        Rectangle {
            anchors.fill: parent
            visible: !Style.sailfish
            gradient: Gradient {
                GradientStop { position: 0; color: Style.bg2 }
                GradientStop { position: 1; color: Style.bg }
            }
        }
    }

    function openIn(path) {
        root = path
        open()
        query.forceActiveFocus()
        query.selectAll()
    }
    onClosed: results.stop()

    SearchModel { id: results }

    property bool content: false
    function run() {
        var q = query.text
        if (q.length === 0)
            return
        var mode = (content ? 2 : 0) + (regex.checked ? 1 : 0)
        results.start(root, q, mode, caseSensitive.checked, hidden.checked)
        Qt.inputMethod.hide()
    }

    contentItem: Item {
        SteelPanel {
            id: top
            anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: -border.width; rightMargin: -border.width; topMargin: -border.width }
            height: col.implicitHeight + 10 * Style.dp
            ledWidth: width
            led: results.running ? "bottom" : ""
            Column {
                id: col
                anchors { left: parent.left; right: parent.right; top: parent.top; topMargin: 4 * Style.dp }
                Item {
                    width: parent.width
                    height: Style.bar
                    IconButton { id: back; anchors { left: parent.left; verticalCenter: parent.verticalCenter } icon: "back"; onClicked: panel.close() }
                    TextField {
                        id: query
                        anchors { left: back.right; right: go.left; verticalCenter: parent.verticalCenter; rightMargin: 4 * Style.dp }
                        font.family: Style.fontFamily
                        color: Style.text
                        font.pixelSize: Style.fontM
                        selectByMouse: true
                        selectionColor: Style.accent
                        placeholderText: panel.content ? Style.t("Testo da cercare nei file", "Text to find in files")
                                                       : Style.t("Nome (anche *.jpg, foto?)", "Name (also *.jpg, photo?)")
                        placeholderTextColor: Style.dim
                        inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                        background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: query.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
                        onAccepted: panel.run()
                    }
                    IconButton {
                        id: go
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        icon: results.running ? "close" : "search"
                        checked: results.running
                        onClicked: results.running ? results.stop() : panel.run()
                    }
                }
                // nome | contenuto
                Row {
                    x: Style.margin
                    spacing: 8 * Style.dp
                    height: 40 * Style.dp
                    SteelPill { anchors.verticalCenter: parent.verticalCenter; text: Style.t("Nel nome", "In name"); role: panel.content ? "" : "accept"; onClicked: panel.content = false }
                    SteelPill { anchors.verticalCenter: parent.verticalCenter; text: Style.t("Nel contenuto", "In content"); role: panel.content ? "accept" : ""; onClicked: panel.content = true }
                }
                Flow {
                    x: Style.margin
                    width: parent.width - 2 * Style.margin
                    CheckRow { id: regex; width: parent.width / 2; text: Style.t("Espressione regolare", "Regular expression") }
                    CheckRow { id: caseSensitive; width: parent.width / 2; text: Style.t("Maiuscole esatte", "Match case") }
                    CheckRow { id: hidden; width: parent.width / 2; text: Style.t("Anche i nascosti", "Include hidden") }
                }
                Text {
                    font.family: Style.fontFamily
                    x: Style.margin
                    width: parent.width - 2 * Style.margin
                    elide: Text.ElideMiddle
                    color: results.error ? Style.danger : Style.dim
                    font.pixelSize: Style.fontS
                    text: results.error ? results.error
                        : results.running ? Style.t("Cerco in ", "Searching ") + results.current
                        : (results.scanned > 0
                           ? Style.count(results.found, Style.t("risultato", "result"), Style.t("risultati", "results"))
                             + Style.t(" su " + results.scanned + " elementi", " in " + results.scanned + " items")
                             + (results.limited ? Style.t(" (fermata al limite)", " (stopped at the limit)") : "")
                           : Style.t("In «", "In “") + (panel.root.split("/").pop() || "/") + Style.t("» e sottocartelle", "” and subfolders"))
                }
            }
        }

        ListView {
            id: list
            anchors { left: parent.left; right: parent.right; top: top.bottom; bottom: parent.bottom }
            clip: true
            model: results
            boundsBehavior: Flickable.StopAtBounds
            delegate: Rectangle {
                width: list.width
                height: Style.row + (snippet ? 16 * Style.dp : 0)
                color: area.pressed ? Style.pressed : "transparent"
                FileIcon {
                    id: icon
                    anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                    width: 40 * Style.dp
                    height: width
                    small: true
                    path: model.path
                    category: model.category
                    thumbnails: false
                }
                Column {
                    anchors { left: icon.right; leftMargin: Style.margin; right: where.left; rightMargin: 6 * Style.dp; verticalCenter: parent.verticalCenter }
                    Text { font.family: Style.fontFamily; width: parent.width; text: name; color: Style.text; font.pixelSize: Style.fontM; elide: Text.ElideMiddle }
                    Text {
                        font.family: Style.fontFamily
                        width: parent.width
                        // il percorso a partire dalla cartella della ricerca
                        text: (parent.parent.relParent || "./") + (isDir ? "" : " · " + Style.size(size))
                        color: Style.dim
                        font.pixelSize: Style.fontS
                        elide: Text.ElideMiddle
                    }
                    Text { font.family: Style.fontFamily; visible: snippet !== ""; width: parent.width; text: snippet; color: Style.accent; font.pixelSize: Style.fontS; elide: Text.ElideRight }
                }
                readonly property string relParent: model.parent.indexOf(panel.root) === 0 ? model.parent.substring(panel.root.length + 1) : model.parent
                MouseArea { id: area; anchors.fill: parent; onClicked: panel.openRequested(results.get(index)) }
                IconButton {
                    id: where
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    icon: "folder"
                    onClicked: panel.showInFolder(model.path)
                }
                Rectangle { anchors { left: icon.right; right: parent.right; bottom: parent.bottom } height: 1; color: Style.line; opacity: 0.6 }
            }
            Text {
                font.family: Style.fontFamily
                anchors.centerIn: parent
                visible: list.count === 0 && !results.running && results.scanned > 0
                text: Style.t("Nessun risultato", "No results")
                color: Style.dim
                font.pixelSize: Style.fontL
            }
        }
    }
}
