import QtQuick
import "."

// Il simbolo di un tipo di connessione su un disco d'acciaio, con l'anello LED
// quando è scelto (come i pulsanti di RooTundra); nel tema Sailfish solo il
// simbolo piatto, del colore di evidenziazione se scelto.
Item {
    id: tb
    property string icon
    property bool lit: false
    width: 50 * Style.dp
    height: width
    SteelDisc { anchors.fill: parent; visible: !Style.sailfish }
    LedRing { anchors.fill: parent; visible: tb.lit && !Style.sailfish }
    Icon {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: Math.max(1, 0.8 * Style.dp)
        visible: !tb.lit && !Style.sailfish
        name: tb.icon ? "luce/" + tb.icon : ""
        size: tb.width * 0.46
    }
    Icon {
        anchors.centerIn: parent
        visible: !Style.sailfish
        name: tb.icon ? (tb.lit ? "acceso/" : "inciso/") + tb.icon : ""
        size: tb.width * 0.46
    }
    Icon {
        anchors.centerIn: parent
        visible: Style.sailfish
        name: tb.icon
        size: tb.width * 0.55
        tint: tb.lit ? Style.sfHighlight : Style.sfPrimary
    }
}
