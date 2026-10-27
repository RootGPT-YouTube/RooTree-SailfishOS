import QtQuick
import "."

// Un pulsante a pastiglia d'acciaio con il bordo LED, come i tasti del
// tastierino di RooTundra. role: "accept" (bordo acceso), "danger" (rosso), altro.
// Nel tema Sailfish è piatto, velato del colore dell'ambience.
Item {
    id: pill
    property string text
    property string role
    signal clicked()
    implicitWidth: Math.max(84 * Style.dp, label.implicitWidth + 30 * Style.dp)
    implicitHeight: 44 * Style.dp
    opacity: enabled ? 1 : 0.4
    readonly property color edge: role === "danger" ? Style.danger : Style.accent
    Rectangle {
        anchors.fill: parent
        radius: height / 2
        readonly property Gradient steel: Gradient {
            GradientStop { position: 0; color: area.pressed ? "#2a3e55" : "#5b6673" }
            GradientStop { position: 0.5; color: area.pressed ? "#1d2c3c" : "#3a434d" }
            GradientStop { position: 1; color: "#20262d" }
        }
        gradient: Style.sailfish ? null : steel
        color: Style.alpha(Style.sfHighlight, area.pressed ? 0.4 : (pill.role === "accept" ? 0.3 : 0.18))
        border.color: Style.sailfish ? (pill.role === "danger" ? Style.danger : "transparent") : pill.role === "accept" || pill.role === "danger" || area.pressed ? pill.edge : Qt.rgba(pill.edge.r, pill.edge.g, pill.edge.b, 0.45)
        border.width: Style.sailfish ? (pill.role === "danger" ? 1.5 * Style.dp : 0) : (pill.role === "accept" || area.pressed ? 2.5 : 1.5) * Style.dp
    }
    Text {
        font.family: Style.fontFamily
        id: label
        anchors.centerIn: parent
        text: pill.text
        color: pill.role === "danger" ? Style.danger : (Style.sailfish && area.pressed ? Style.sfHighlight : Style.text)
        font.pixelSize: Style.fontM
    }
    MouseArea { id: area; anchors.fill: parent; onClicked: pill.clicked() }
}
