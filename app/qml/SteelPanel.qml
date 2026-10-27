import QtQuick
import "."

// Un pannello d'acciaio PIENO, come il tastierino della schermata di blocco di
// RooTundra (l'utente: quello trasparente sopra lo sfondo non si leggeva),
// con il filo LED blu su un bordo. Il LED non «respira»: un'animazione
// continua in un'app che resta aperta a lungo costerebbe batteria.
// Nel tema Sailfish: una velatura del colore dell'ambience sopra lo sfondo
// (barre) o un pannello scuro velato (`overlay`: fogli, dialoghi, avvisi).
Rectangle {
    id: p
    // "top", "bottom" o "" (nessun filo)
    property string led: "top"
    property real ledWidth: width * 0.4
    property bool overlay: false
    readonly property Gradient steel: Gradient {
        GradientStop { position: 0; color: "#3c4651" }
        GradientStop { position: 0.08; color: "#2a3139" }
        GradientStop { position: 1; color: "#14181d" }
    }
    gradient: Style.sailfish ? null : steel
    color: overlay ? Style.sfOverlay : Style.sfBar
    border.color: Style.sailfish ? Style.alpha(Style.sfHighlight, 0.3) : "#6b7b8c"
    border.width: Style.sailfish && !overlay ? 0 : Math.max(1, 1.2 * Style.dp)
    Rectangle {
        visible: p.led !== "" && !Style.sailfish
        anchors.horizontalCenter: parent.horizontalCenter
        y: p.led === "top" ? 6 * Style.dp : parent.height - height - 1
        width: p.ledWidth
        height: Math.max(2, 2.2 * Style.dp)
        radius: height / 2
        color: Style.accent
        opacity: 0.8
    }
}
