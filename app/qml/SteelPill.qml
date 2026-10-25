import QtQuick
import "."

// Un pulsante a pastiglia d'acciaio con il bordo LED, come i tasti del
// tastierino di RooTundra. role: "accept" (bordo acceso), "danger" (rosso), altro.
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
        gradient: Gradient {
            GradientStop { position: 0; color: area.pressed ? "#2a3e55" : "#5b6673" }
            GradientStop { position: 0.5; color: area.pressed ? "#1d2c3c" : "#3a434d" }
            GradientStop { position: 1; color: "#20262d" }
        }
        border.color: pill.role === "accept" || pill.role === "danger" || area.pressed ? pill.edge : Qt.rgba(pill.edge.r, pill.edge.g, pill.edge.b, 0.45)
        border.width: (pill.role === "accept" || area.pressed ? 2.5 : 1.5) * Style.dp
    }
    Text {
        id: label
        anchors.centerIn: parent
        text: pill.text
        color: pill.role === "danger" ? Style.danger : Style.text
        font.pixelSize: Style.fontM
    }
    MouseArea { id: area; anchors.fill: parent; onClicked: pill.clicked() }
}
