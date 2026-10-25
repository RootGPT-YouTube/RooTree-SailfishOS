import QtQuick
import "."

// La barra in alto dei visori: chiudi, titolo e sottotitolo, poi le azioni.
SteelPanel {
    id: bar
    property string title
    property string subtitle
    default property alias actions: actionsRow.data
    signal closeClicked()
    led: "bottom"
    ledWidth: width * 0.4
    height: Style.bar + 6 * Style.dp
    IconButton {
        id: closeButton
        anchors { left: parent.left; verticalCenter: parent.verticalCenter }
        icon: "back"
        onClicked: bar.closeClicked()
    }
    Column {
        anchors { left: closeButton.right; right: actionsRow.left; verticalCenter: parent.verticalCenter; rightMargin: 6 * Style.dp }
        Text { width: parent.width; text: bar.title; color: Style.text; font.pixelSize: Style.fontM; elide: Text.ElideMiddle }
        Text { width: parent.width; visible: text !== ""; text: bar.subtitle; color: Style.dim; font.pixelSize: Style.fontS; elide: Text.ElideRight }
    }
    Row {
        id: actionsRow
        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
        height: parent.height
    }
}
