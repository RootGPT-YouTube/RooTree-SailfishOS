import QtQuick
import "."

// Il titolo di un gruppo di righe in un menù.
Item {
    property string text
    width: parent ? parent.width : 0
    height: 34 * Style.dp
    Text {
        font.family: Style.fontFamily
        anchors.left: parent.left
        anchors.leftMargin: Style.margin
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 6 * Style.dp
        text: parent.text.toUpperCase()
        color: Style.accent
        font.pixelSize: 10.5 * Style.dp
        font.bold: true
        font.letterSpacing: 1.2 * Style.dp
    }
}
