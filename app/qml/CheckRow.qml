import QtQuick
import "."

// Una casella con la sua scritta.
Item {
    id: c
    property string text
    property bool checked: false
    width: parent ? parent.width : 0
    height: 38 * Style.dp
    Rectangle {
        id: box
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: 22 * Style.dp
        height: width
        radius: 5 * Style.dp
        // una casella d'acciaio; spuntata, il LED blu si accende dentro
        readonly property Gradient steel: Gradient {
            GradientStop { position: 0; color: c.checked ? "#3b8cff" : "#5b6673" }
            GradientStop { position: 1; color: c.checked ? "#0a55d0" : "#20262d" }
        }
        // Sailfish: piena del colore di evidenziazione quando è spuntata
        gradient: Style.sailfish ? null : steel
        color: c.checked ? Style.alpha(Style.sfHighlight, 0.6) : "transparent"
        border.color: Style.sailfish ? Style.sfHighlight : (c.checked ? "#9fd0ff" : Style.steelBorder)
        border.width: 1.5 * Style.dp
        Icon { anchors.centerIn: parent; visible: c.checked; name: "check"; size: 14 * Style.dp }
    }
    Text {
        font.family: Style.fontFamily
        anchors.left: box.right
        anchors.leftMargin: 10 * Style.dp
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: c.text
        color: Style.text
        font.pixelSize: Style.fontM
        elide: Text.ElideRight
    }
    MouseArea { anchors.fill: parent; onClicked: c.checked = !c.checked }
}
