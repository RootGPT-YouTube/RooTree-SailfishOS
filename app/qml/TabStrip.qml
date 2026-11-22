import QtQuick
import "."

// Le due schede in cima (0.9.8): due linguette come quelle di due cartellette
// d'archivio. Quella in vista è alta, piena e col filo LED; l'altra è più
// bassa e spenta. Un tocco la porta in vista (anche lo swipe sulla cartella).
Item {
    id: strip
    property int current: 0
    property var labels: ["", ""]
    property var icons: ["", ""]
    signal picked(int i)
    height: 38 * Style.dp

    Repeater {
        model: 2
        delegate: SteelPanel {
            id: tabItem
            readonly property bool active: index === strip.current
            readonly property real gap: 6 * Style.dp
            x: gap + index * (width + gap)
            width: (strip.width - 3 * gap) / 2
            // quella in vista scende di `sink` sulla barra sotto e ne copre il
            // filo chiaro in cima: linguetta e barra sono un pezzo solo
            readonly property real sink: active && !Style.sailfish ? 6 * Style.dp : 0
            height: (active ? strip.height - 3 * Style.dp : strip.height - 10 * Style.dp) + sink
            y: strip.height - height + sink
            // l'acciaio della linguetta in vista finisce del colore della barra sotto
            readonly property Gradient joined: Gradient {
                GradientStop { position: 0; color: "#3c4651" }
                GradientStop { position: 0.12; color: "#2a3139" }
                GradientStop { position: 1; color: "#2a3139" }
            }
            gradient: Style.sailfish ? null : (active ? joined : steel)
            topLeftRadius: 12 * Style.dp
            topRightRadius: 12 * Style.dp
            bottomLeftRadius: 0
            bottomRightRadius: 0
            led: active ? "top" : ""
            ledWidth: width * 0.45
            opacity: active ? 1 : 0.55
            Behavior on height { NumberAnimation { duration: 120 } }

            // la lettera della scheda, in un dischetto
            Rectangle {
                id: badge
                anchors { left: parent.left; leftMargin: 9 * Style.dp; verticalCenter: parent.verticalCenter; verticalCenterOffset: (tabItem.active ? 2 * Style.dp : 0) - tabItem.sink / 2 }
                width: 19 * Style.dp
                height: width
                radius: width / 2
                color: tabItem.active ? Style.accent : "transparent"
                border.color: Style.accent
                border.width: Math.max(1, 1.2 * Style.dp)
                Text {
                    anchors.centerIn: parent
                    font.family: Style.fontFamily
                    text: index === 0 ? "A" : "B"
                    color: Style.text
                    font.pixelSize: 11 * Style.dp
                    font.bold: true
                }
            }
            Icon {
                id: tabIcon
                anchors { left: badge.right; leftMargin: 6 * Style.dp; verticalCenter: badge.verticalCenter }
                name: strip.icons[index] || ""
                size: 16 * Style.dp
            }
            Text {
                anchors { left: tabIcon.right; leftMargin: 6 * Style.dp; right: parent.right; rightMargin: 9 * Style.dp; verticalCenter: badge.verticalCenter }
                font.family: Style.fontFamily
                text: strip.labels[index] || ""
                color: Style.text
                font.pixelSize: Style.fontS
                font.bold: tabItem.active
                elide: Text.ElideMiddle
            }
            // si tocca anche lo spazio sopra la linguetta bassa
            MouseArea {
                anchors { fill: parent; topMargin: -(strip.height - tabItem.height) }
                onClicked: strip.picked(index)
            }
        }
    }
    // copre il bordo in fondo alla linguetta in vista (e il filo chiaro della
    // barra sotto): da fratello, sopra la linguetta, così nessun bordo passa
    Rectangle {
        readonly property real gap: 6 * Style.dp
        readonly property real bw: Math.max(1, 1.2 * Style.dp)
        readonly property real tabW: (strip.width - 3 * gap) / 2
        visible: !Style.sailfish
        x: gap + strip.current * (tabW + gap) + bw
        width: tabW - 2 * bw
        y: strip.height - 1
        height: 6 * Style.dp + 2
        color: "#2a3139"
    }
}
