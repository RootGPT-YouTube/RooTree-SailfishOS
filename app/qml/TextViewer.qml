import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Il visore del testo, che diventa editor con «Modifica»: a capo automatico
// o no, i file troppo grandi si mostrano tagliati (con l'avviso, e allora non
// si modificano: si salverebbe un file troncato), quelli binari no. Si salva
// senza mai lasciare il file a metà; da root l'originale va nelle copie di
// sicurezza (Fase 4: il criterio è modificare un file di /etc).
Popup {
    id: viewer
    property string path
    property var content: ({})
    property bool wrap: true
    property bool editing: false
    property bool dirty: false
    readonly property bool canEdit: content.text !== undefined && !content.truncated
                                    && (Fs.isWritable(path) || Fs.rootActive)
    signal openWithRequested(string path)
    signal message(string text, bool isError)
    signal saved()

    parent: Overlay.overlay
    x: 0; y: 0
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    padding: 0
    modal: true
    closePolicy: Popup.NoAutoClose
    background: Rectangle { color: Style.bg }

    function openFor(p) {
        path = p
        content = Opener.readText(p)
        editing = false
        dirty = false
        open()
        flick.contentY = 0
        flick.contentX = 0
    }

    function save() {
        // l'ultima parola scritta è ancora «in composizione» nella tastiera:
        // senza commit() resterebbe fuori dal file (visto il 10/11)
        Qt.inputMethod.commit()
        var r = Fs.writeText(path, body.text)
        if (r.error) {
            message(Style.t("Non salvato: ", "Not saved: ") + r.error, true)
            return false
        }
        dirty = false
        message(r.backup ? Style.t("Salvato. L'originale è in ", "Saved. The original is in ") + r.backup
                         : Style.t("Salvato", "Saved"), false)
        saved()
        return true
    }

    AppDialog {
        id: unsaved
        title: Style.t("Modifiche non salvate", "Unsaved changes")
        message: Style.t("Salvare «" + viewer.path.split("/").pop() + "» prima di chiudere?", "Save “" + viewer.path.split("/").pop() + "” before closing?")
        buttons: [{ text: Style.t("Annulla", "Cancel"), role: "reject" }, { text: Style.t("Scarta", "Discard"), role: "danger" }, { text: Style.t("Salva", "Save"), role: "accept" }]
        onChosen: function (role) {
            if (role === "danger") { viewer.dirty = false; viewer.close() }
            else if (role === "accept" && viewer.save()) viewer.close()
        }
    }

    contentItem: Item {
        ViewerBar {
            id: bar
            anchors { left: parent.left; right: parent.right; top: parent.top }
            title: viewer.path.split("/").pop()
            subtitle: viewer.editing ? (viewer.dirty ? Style.t("Modificato, non salvato", "Modified, not saved") : Style.t("Modifica", "Editing"))
                      : viewer.content.truncated ? Style.t("Mostrati i primi 2 MB", "Showing the first 2 MB") : ""
            onCloseClicked: viewer.dirty ? unsaved.open() : viewer.close()
            IconButton { icon: "wrap"; checked: viewer.wrap; onClicked: viewer.wrap = !viewer.wrap }
            IconButton { visible: viewer.canEdit && !viewer.editing; icon: "rename"; onClicked: { viewer.editing = true; body.forceActiveFocus() } }
            IconButton { visible: viewer.editing; icon: "check"; checked: viewer.dirty; onClicked: viewer.save() }
            IconButton { visible: !viewer.editing; icon: "share"; onClicked: Opener.share([viewer.path]) }
            IconButton { visible: !viewer.editing; icon: "openwith"; onClicked: viewer.openWithRequested(viewer.path) }
        }
        Flickable {
            id: flick
            anchors { left: parent.left; right: parent.right; top: bar.bottom; bottom: parent.bottom }
            // sopra la tastiera, quando c'è
            anchors.bottomMargin: Qt.inputMethod.visible ? Qt.inputMethod.keyboardRectangle.height : 0
            contentWidth: viewer.wrap ? width : Math.max(width, body.implicitWidth + 2 * Style.margin)
            contentHeight: body.implicitHeight + 2 * Style.margin
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            visible: viewer.content.text !== undefined
            TextEdit {
                id: body
                x: Style.margin
                y: Style.margin
                width: viewer.wrap ? flick.width - 2 * Style.margin : implicitWidth
                readOnly: !viewer.editing
                selectByMouse: viewer.editing
                persistentSelection: true
                onTextChanged: if (viewer.editing) viewer.dirty = true
                // il cursore resta in vista mentre si scrive
                onCursorRectangleChanged: {
                    if (!viewer.editing) return
                    var r = cursorRectangle
                    if (r.y + y < flick.contentY) flick.contentY = r.y + y - Style.margin
                    else if (r.y + y + r.height > flick.contentY + flick.height) flick.contentY = r.y + y + r.height - flick.height + Style.margin
                }
                textFormat: TextEdit.PlainText
                wrapMode: viewer.wrap ? TextEdit.WrapAtWordBoundaryOrAnywhere : TextEdit.NoWrap
                text: viewer.content.text || ""
                color: Style.text
                font.family: "monospace"
                font.pixelSize: 12.5 * Style.dp
            }
        }
        Column {
            visible: viewer.content.text === undefined
            anchors.centerIn: parent
            width: parent.width * 0.8
            spacing: 12 * Style.dp
            Text {
                font.family: Style.fontFamily
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: viewer.content.binary ? Style.t("Non è un file di testo.", "This is not a text file.")
                                            : (viewer.content.error || "")
                color: Style.text
                font.pixelSize: Style.fontL
            }
            SteelPill {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Style.t("Apri con…", "Open with…")
                onClicked: viewer.openWithRequested(viewer.path)
            }
        }
    }
}
