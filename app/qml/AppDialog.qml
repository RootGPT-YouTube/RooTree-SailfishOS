import QtQuick
import "."
import QtQuick.Controls.Basic

// Un dialogo al centro, con titolo, testo, contenuto libero e una fila di
// pulsanti. `buttons`: [{text, role}] dove role = "accept" | "reject" | altro;
// alla pressione si emette `chosen(role)` e il dialogo si chiude.
Popup {
    id: d
    property string title
    property string message
    property var buttons: []
    default property alias content: extra.data
    signal chosen(string role)

    parent: Overlay.overlay
    modal: true
    dim: true
    closePolicy: Popup.NoAutoClose
    width: Math.min(parent ? parent.width - 2 * Style.margin : 0, 380 * Style.dp)
    x: parent ? (parent.width - width) / 2 : 0
    y: parent ? Math.max(Style.margin, (parent.height - height) / 2 - 40 * Style.dp) : 0
    padding: Style.margin

    Overlay.modal: Rectangle { color: "#99000000" }
    background: SteelPanel { overlay: true; radius: 18 * Style.dp; ledWidth: 110 * Style.dp }

    contentItem: Column {
        spacing: 10 * Style.dp
        topPadding: 10 * Style.dp
        Text {
            font.family: Style.fontFamily
            width: parent.width
            visible: d.title !== ""
            text: d.title
            color: Style.text
            font.pixelSize: Style.fontL
            font.bold: true
            wrapMode: Text.Wrap
        }
        Text {
            font.family: Style.fontFamily
            width: parent.width
            visible: d.message !== ""
            text: d.message
            color: Style.dim
            font.pixelSize: Style.fontM
            wrapMode: Text.Wrap
        }
        Column {
            id: extra
            width: parent.width
            spacing: 8 * Style.dp
        }
        Flow {
            width: parent.width
            layoutDirection: Qt.RightToLeft
            spacing: 6 * Style.dp
            Repeater {
                model: d.buttons
                delegate: SteelPill {
                    text: modelData.text
                    role: modelData.role
                    // la tastiera di Sailfish tiene l'ultima parola «in composizione»
                    // finché non arriva uno spazio: senza commit() il campo non la ha
                    onClicked: { Qt.inputMethod.commit(); d.close(); d.chosen(modelData.role) }
                }
            }
        }
    }
}
