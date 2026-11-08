import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Proprietà di uno o più elementi: tipo, dimensione (per le cartelle la conta
// il nucleo in background), data, link, proprietario e permessi. I permessi si
// cambiano qui se l'elemento è dell'utente; gli altri richiederanno il root.
Sheet {
    id: sheet
    property var paths: []
    property var info: ({})
    property int sizeJob: 0
    property var sizeStatus: ({})
    property int mode: 0
    property string error
    readonly property bool single: paths.length === 1
    // i permessi si cambiano se l'elemento è tuo, o da root; il proprietario solo da root
    readonly property bool inArchive: single && info.path !== undefined && Fs.archiveOf(info.path) !== ""
    readonly property bool onServer: single && (info.path || "").indexOf("/@remote/") === 0
    // sul server i permessi li giudica il server (uid locale e remoto non si confrontano)
    readonly property bool canEdit: single && !inArchive && (info.mine === true || Fs.rootActive || onServer)

    title: single ? (info.name || "") : Style.count(paths.length, Style.t("elemento", "item"), Style.t("elementi", "items"))

    function openFor(list) {
        paths = list
        info = single ? Fs.info(list[0]) : ({})
        mode = single ? (info.mode & 4095) : 0
        ownerUser.text = info.user || ""
        ownerGroup.text = info.group || ""
        error = ""
        sizeStatus = ({})
        sizeJob = (!single || info.isDir) ? Jobs.size(list) : 0
        open()
    }
    onClosed: {
        if (sizeJob) {
            Jobs.cancel(sizeJob)
            Jobs.forget(sizeJob)
            sizeJob = 0
        }
    }
    Connections {
        target: Jobs
        function onJobsChanged() {
            if (sheet.sizeJob)
                sheet.sizeStatus = Jobs.status(sheet.sizeJob)
        }
    }

    function typeText() {
        if (info.isLink)
            return info.linkBroken ? Style.t("Collegamento rotto", "Broken link") : Style.t("Collegamento", "Link")
        if (info.isDir)
            return Style.t("Cartella", "Folder")
        if (info.kind === 2)
            return Style.t("File speciale", "Special file")
        var ext = info.name.indexOf(".") > 0 ? info.name.split(".").pop().toUpperCase() : ""
        return ext ? Style.t("File " + ext, ext + " file") : Style.t("File", "File")
    }
    function sizeText() {
        if (single && !info.isDir)
            return Style.size(info.size) + (info.size >= 1000 ? " (" + Number(info.size).toLocaleString(Qt.locale(), 'f', 0) + " byte)" : "")
        var s = sizeStatus
        if (!s.id || s.state < 5)
            return Style.t("Calcolo in corso…", "Calculating…")
        return Style.size(s.totalBytes) + " · " + Style.count(s.totalItems, Style.t("file", "file"), Style.t("file", "files"))
               + ", " + Style.count(Math.max(0, s.dirs - (single ? 1 : 0)), Style.t("cartella", "folder"), Style.t("cartelle", "folders"))
    }

    component Field: Item {
        property string label
        property string value
        width: parent.width
        height: Math.max(36 * Style.dp, v.implicitHeight + 12 * Style.dp)
        Text {
            font.family: Style.fontFamily
            id: l
            x: Style.margin
            width: 110 * Style.dp
            anchors.top: parent.top
            anchors.topMargin: 6 * Style.dp
            text: parent.label
            color: Style.dim
            font.pixelSize: Style.fontS
        }
        Text {
            font.family: Style.fontFamily
            id: v
            anchors.left: l.right
            anchors.right: parent.right
            anchors.rightMargin: Style.margin
            anchors.top: parent.top
            anchors.topMargin: 5 * Style.dp
            text: parent.value
            color: Style.text
            font.pixelSize: Style.fontM
            wrapMode: Text.WrapAnywhere
        }
    }

    Field { visible: sheet.single; label: Style.t("Tipo", "Type"); value: sheet.single ? sheet.typeText() : "" }
    Field { label: Style.t("Dimensione", "Size"); value: sheet.sizeText() }
    Field { visible: sheet.single; label: Style.t("Percorso", "Location"); value: sheet.single ? Fs.parentOf(sheet.info.path || "/") : "" }
    Field { visible: sheet.single && !!sheet.info.isLink; label: Style.t("Punta a", "Points to"); value: sheet.info.linkTarget || "" }
    Field { visible: sheet.single; label: Style.t("Modificato", "Modified"); value: Style.date(sheet.info.mtime) }
    Field { visible: sheet.single; label: Style.t("Proprietario", "Owner"); value: (sheet.info.user || "") + " : " + (sheet.info.group || "") }
    Field {
        visible: sheet.single
        label: Style.t("Permessi", "Permissions")
        value: Style.modeString(sheet.mode) + "   " + ("000" + sheet.mode.toString(8)).slice(-4)
    }

    // la griglia dei permessi: proprietario, gruppo, altri × lettura, scrittura, esecuzione
    Grid {
        visible: sheet.canEdit
        x: Style.margin
        columns: 4
        rowSpacing: 0
        columnSpacing: 6 * Style.dp
        Repeater {
            model: [Style.t("", ""), Style.t("Lettura", "Read"), Style.t("Scrittura", "Write"), Style.t("Esecuzione", "Exec")]
            delegate: Text { font.family: Style.fontFamily; width: index === 0 ? 92 * Style.dp : 76 * Style.dp; text: modelData; color: Style.dim; font.pixelSize: Style.fontS }
        }
        Repeater {
            model: 12
            delegate: Item {
                readonly property int who: Math.floor(index / 4)
                readonly property int what: index % 4 - 1
                readonly property int bit: what < 0 ? 0 : (256 >> (who * 3 + what))
                width: what < 0 ? 92 * Style.dp : 76 * Style.dp
                height: 38 * Style.dp
                Text {
                    font.family: Style.fontFamily
                    visible: parent.what < 0
                    anchors.verticalCenter: parent.verticalCenter
                    text: [Style.t("Proprietario", "Owner"), Style.t("Gruppo", "Group"), Style.t("Altri", "Others")][parent.who]
                    color: Style.text
                    font.pixelSize: Style.fontS
                }
                CheckRow {
                    visible: parent.what >= 0
                    width: 30 * Style.dp
                    anchors.verticalCenter: parent.verticalCenter
                    checked: (sheet.mode & parent.bit) !== 0
                    onCheckedChanged: sheet.mode = checked ? (sheet.mode | parent.bit) : (sheet.mode & ~parent.bit)
                }
            }
        }
    }
    Text {
        font.family: Style.fontFamily
        visible: sheet.single && !sheet.canEdit && !sheet.inArchive
        x: Style.margin
        width: parent.width - 2 * Style.margin
        wrapMode: Text.Wrap
        text: Style.t("Non è tuo: per cambiarne i permessi accendi la modalità root (menù ⋮).",
                      "Not yours: to change its permissions turn on root mode (⋮ menu).")
        color: Style.dim
        font.pixelSize: Style.fontS
    }
    Text {
        font.family: Style.fontFamily
        visible: sheet.error !== ""
        x: Style.margin
        width: parent.width - 2 * Style.margin
        wrapMode: Text.Wrap
        text: sheet.error
        color: Style.danger
        font.pixelSize: Style.fontS
    }
    MenuRow {
        visible: sheet.canEdit && sheet.mode !== (sheet.info.mode & 4095)
        icon: "check"
        text: Style.t("Applica i permessi", "Apply permissions")
        onClicked: {
            sheet.error = Fs.setMode(sheet.info.path, sheet.mode)
            if (sheet.error === "")
                sheet.info = Fs.info(sheet.info.path)
        }
    }

    // il proprietario, solo da root (nomi o numeri)
    Row {
        visible: sheet.single && Fs.rootActive && !sheet.inArchive && !sheet.onServer
        x: Style.margin
        spacing: 8 * Style.dp
        component OwnerField: TextField {
            font.family: Style.fontFamily
            width: (sheet.width - 2 * Style.margin - 8 * Style.dp) / 2
            color: Style.text
            font.pixelSize: Style.fontM
            inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
            placeholderTextColor: Style.dim
            background: Rectangle { color: Style.bg; radius: 6 * Style.dp; border.color: parent && parent.activeFocus ? Style.accent : Style.line; border.width: 1.5 * Style.dp }
        }
        OwnerField { id: ownerUser; placeholderText: Style.t("Proprietario", "Owner") }
        OwnerField { id: ownerGroup; placeholderText: Style.t("Gruppo", "Group") }
    }
    MenuRow {
        visible: sheet.single && Fs.rootActive && !sheet.inArchive && !sheet.onServer
                 && (ownerUser.text !== (sheet.info.user || "") || ownerGroup.text !== (sheet.info.group || ""))
        icon: "check"
        text: Style.t("Applica il proprietario", "Apply owner")
        onClicked: {
            sheet.error = Fs.setOwner(sheet.info.path, ownerUser.text.trim(), ownerGroup.text.trim())
            if (sheet.error === "") {
                sheet.info = Fs.info(sheet.info.path)
                ownerUser.text = sheet.info.user || ""
                ownerGroup.text = sheet.info.group || ""
            }
        }
    }
}
