import QtQuick
import "."
import QtQuick.Controls.Basic

// Un pannello che sale dal basso, sopra uno sfondo scurito. Il contenuto va
// in `content` (una colonna); se è più alto dello schermo scorre.
Popup {
    id: sheet
    default property alias content: col.data
    property string title

    parent: Overlay.overlay
    modal: true
    dim: true
    x: 0
    width: parent ? parent.width : 0
    y: parent ? parent.height - height : 0
    height: Math.min(parent ? parent.height * 0.88 : 0, flick.contentHeight + header.height + Style.margin)
    padding: 0
    closePolicy: Popup.CloseOnPressOutside

    Overlay.modal: Rectangle { color: "#99000000" }

    // un pannello riusato (la pagina «i» delle memorie) si riapre dall'inizio,
    // non dove lo si era lasciato
    onAboutToShow: flick.contentY = 0

    // dissolvenza e non scorrimento: un'animazione su y ne romperebbe il
    // legame con l'altezza, che si conosce solo dopo aver steso il contenuto
    // (il pannello restava a metà fuori dallo schermo)
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 140 } }
    exit: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 100 } }

    // il pannello d'acciaio pieno del tastierino di RooTundra, con il filo LED
    // in alto; gli angoli in basso restano sotto il bordo dello schermo
    background: Item {
        clip: true
        SteelPanel {
            overlay: true
            anchors.fill: parent
            anchors.bottomMargin: -radius
            radius: 22 * Style.dp
            ledWidth: 120 * Style.dp
        }
    }

    contentItem: Item {
        Item {
            id: header
            width: parent.width
            height: sheet.title ? 44 * Style.dp : 18 * Style.dp
            Text {
                font.family: Style.fontFamily
                visible: sheet.title !== ""
                anchors.left: parent.left
                anchors.leftMargin: Style.margin
                anchors.right: parent.right
                anchors.rightMargin: Style.margin
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 4 * Style.dp
                text: sheet.title
                color: Style.text
                font.pixelSize: Style.fontL
                font.bold: true
                elide: Text.ElideMiddle
            }
        }
        Flickable {
            id: flick
            anchors.top: header.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            contentHeight: col.implicitHeight + Style.margin
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: col
                width: flick.width
            }
        }
    }
}
