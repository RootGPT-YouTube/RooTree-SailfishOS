import QtQuick
import "."

// Una riga di un menù: icona, testo, a destra un segno di spunta o un dettaglio.
// Con `expandable` c'è in fondo una freccia che apre e chiude le righe figlie
// (segnale `toggled`), separata dal tocco sulla riga.
Rectangle {
    id: m
    property string icon
    property string text
    property string detail
    property bool checked: false
    property bool checkable: false
    property bool indent: false
    property int depth: indent ? 1 : 0
    // un'immagine di icons/ (es. "tipi/folder-music.png") al posto del simbolo
    property string image
    property bool expandable: false
    property bool expanded: false
    signal clicked()
    signal toggled()

    width: parent ? parent.width : 0
    height: Style.row
    color: area.pressed ? Style.pressed : "transparent"
    opacity: enabled ? 1 : 0.4

    readonly property bool hasIcon: m.icon !== "" || m.image !== ""
    Icon {
        id: ic
        visible: m.hasIcon
        anchors.left: parent.left
        anchors.leftMargin: Style.margin + m.depth * 22 * Style.dp
        anchors.verticalCenter: parent.verticalCenter
        name: m.image ? "" : m.icon
        source: m.image ? Qt.resolvedUrl("icons/" + m.image) : (name ? Qt.resolvedUrl("icons/" + name + ".svg") : "")
        size: (m.image ? 24 : 20) * Style.dp
        // le cartelle (immagini) come nella griglia: del colore di evidenziazione
        tint: m.image ? Style.sfHighlight : Style.iconTint
    }
    Text {
        font.family: Style.fontFamily
        anchors.left: m.hasIcon ? ic.right : parent.left
        anchors.leftMargin: m.hasIcon ? Style.margin : Style.margin + m.depth * 22 * Style.dp
        anchors.right: right.left
        anchors.rightMargin: 8 * Style.dp
        anchors.verticalCenter: parent.verticalCenter
        text: m.text
        color: Style.text
        font.pixelSize: Style.fontM
        elide: Text.ElideRight
    }
    Item {
        id: right
        anchors.right: m.expandable ? toggle.left : parent.right
        anchors.rightMargin: m.expandable ? 0 : Style.margin
        anchors.verticalCenter: parent.verticalCenter
        width: m.checkable ? 22 * Style.dp : detailText.implicitWidth
        height: parent.height
        Icon {
            anchors.centerIn: parent
            visible: m.checkable && m.checked
            name: "check"
            size: 18 * Style.dp
        }
        Text {
            font.family: Style.fontFamily
            id: detailText
            anchors.verticalCenter: parent.verticalCenter
            anchors.right: parent.right
            visible: !m.checkable
            text: m.detail
            color: Style.dim
            font.pixelSize: Style.fontS
        }
    }
    MouseArea {
        id: area
        anchors.fill: parent
        onClicked: m.clicked()
    }
    // la freccia ha una zona di tocco sua, larga quanto un pulsante
    Rectangle {
        id: toggle
        visible: m.expandable
        anchors.right: parent.right
        width: m.expandable ? Style.row : 0
        height: parent.height
        color: toggleArea.pressed ? Style.pressed : "transparent"
        Icon {
            anchors.centerIn: parent
            name: "down"
            size: 14 * Style.dp
            rotation: m.expanded ? 180 : 0
            opacity: 0.8
            Behavior on rotation { NumberAnimation { duration: 120 } }
        }
        MouseArea {
            id: toggleArea
            anchors.fill: parent
            onClicked: m.toggled()
        }
    }
}
