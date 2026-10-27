import QtQuick
import RooTree
import "."

// «Apri con»: le app di Sailfish che aprono questo tipo di file (come le
// chiede libcontentaction), con la possibilità di farne la predefinita.
Sheet {
    id: sheet
    property string path
    property string mime
    property var apps: []
    title: Style.t("Apri con", "Open with")

    function openFor(p) {
        path = p
        mime = Opener.mimeType(p)
        apps = Opener.actions(p)
        makeDefault.checked = false
        open()
    }

    Text {
        font.family: Style.fontFamily
        x: Style.margin
        width: parent.width - 2 * Style.margin
        text: path.split("/").pop() + " · " + mime
        color: Style.dim
        font.pixelSize: Style.fontS
        elide: Text.ElideMiddle
        bottomPadding: 6 * Style.dp
    }
    Text {
        font.family: Style.fontFamily
        visible: sheet.apps.length === 0
        x: Style.margin
        width: parent.width - 2 * Style.margin
        height: Style.row
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.Wrap
        text: Style.t("Nessuna app installata apre questo tipo di file.", "No installed app opens this kind of file.")
        color: Style.text
        font.pixelSize: Style.fontM
    }
    Repeater {
        model: sheet.apps
        delegate: Rectangle {
            width: parent.width
            height: Style.row + 8 * Style.dp
            color: area.pressed ? Style.pressed : "transparent"
            Image {
                id: icon
                anchors { left: parent.left; leftMargin: Style.margin; verticalCenter: parent.verticalCenter }
                width: 40 * Style.dp
                height: width
                source: modelData.icon
                sourceSize: Qt.size(width, height)
                visible: modelData.icon !== ""
            }
            Text {
                font.family: Style.fontFamily
                anchors { left: icon.right; leftMargin: Style.margin; right: def.left; verticalCenter: parent.verticalCenter }
                text: modelData.name
                color: Style.text
                font.pixelSize: Style.fontM
                elide: Text.ElideRight
            }
            Text {
                font.family: Style.fontFamily
                id: def
                anchors { right: parent.right; rightMargin: Style.margin; verticalCenter: parent.verticalCenter }
                text: modelData.isDefault ? Style.t("predefinita", "default") : ""
                color: Style.accent
                font.pixelSize: Style.fontS
            }
            MouseArea {
                id: area
                anchors.fill: parent
                onClicked: {
                    if (makeDefault.checked)
                        Opener.setDefault(sheet.mime, modelData.id)
                    Opener.openWith(sheet.path, modelData.id)
                    sheet.close()
                }
            }
        }
    }
    Item {
        visible: sheet.apps.length > 1
        width: parent.width
        height: 44 * Style.dp
        CheckRow {
            id: makeDefault
            x: Style.margin
            width: parent.width - 2 * Style.margin
            text: Style.t("Usa sempre per i file " + sheet.mime, "Always use for " + sheet.mime + " files")
        }
    }
}
