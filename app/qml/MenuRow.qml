import QtQuick
import "."

// Una riga di un menù: icona, testo, a destra un segno di spunta o un dettaglio.
Rectangle {
    id: m
    property string icon
    property string text
    property string detail
    property bool checked: false
    property bool checkable: false
    property bool indent: false
    signal clicked()

    width: parent ? parent.width : 0
    height: Style.row
    color: area.pressed ? Style.pressed : "transparent"
    opacity: enabled ? 1 : 0.4

    Icon {
        id: ic
        visible: m.icon !== ""
        anchors.left: parent.left
        anchors.leftMargin: Style.margin + (m.indent ? 22 * Style.dp : 0)
        anchors.verticalCenter: parent.verticalCenter
        name: m.icon
        size: 20 * Style.dp
    }
    Text {
        anchors.left: m.icon !== "" ? ic.right : parent.left
        anchors.leftMargin: m.icon !== "" ? Style.margin : Style.margin + (m.indent ? 22 * Style.dp : 0)
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
        anchors.right: parent.right
        anchors.rightMargin: Style.margin
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
}
