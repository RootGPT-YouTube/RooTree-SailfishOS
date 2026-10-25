import QtQuick
import "."

// Un pulsante a disco d'acciaio come quelli del centro di controllo di
// RooTundra: il simbolo è inciso scuro nell'acciaio (con un filo di luce
// sotto); premuto o attivo, si accende l'anello LED blu e il simbolo si
// illumina. Se c'è `label`, la scritta va sotto il disco.
Item {
    id: b
    property string icon
    property string label
    property bool checked: false
    property real discScale: 0.8
    signal clicked()
    signal pressAndHold()

    readonly property bool lit: checked || area.pressed
    readonly property real labelRoom: label ? 15 * Style.dp : 0
    readonly property real disc: Math.min(width, height - labelRoom) * discScale

    implicitWidth: Style.bar
    implicitHeight: Style.bar
    opacity: enabled ? 1 : 0.4

    Item {
        id: face
        width: b.disc
        height: b.disc
        anchors.horizontalCenter: parent.horizontalCenter
        y: (b.height - b.labelRoom - b.disc) / 2
        scale: area.pressed ? 0.94 : 1
        Behavior on scale { NumberAnimation { duration: 80 } }
        SteelDisc { anchors.fill: parent }
        LedRing { anchors.fill: parent; opacity: b.lit ? 1 : 0; visible: opacity > 0; Behavior on opacity { NumberAnimation { duration: 160 } } }
        Icon {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: Math.max(1, 0.8 * Style.dp)
            visible: !b.lit
            name: b.icon ? "luce/" + b.icon : ""
            size: b.disc * 0.44
        }
        Icon {
            anchors.centerIn: parent
            name: b.icon ? (b.lit ? "acceso/" : "inciso/") + b.icon : ""
            size: b.disc * 0.44
        }
    }
    Text {
        visible: b.label !== ""
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 1 * Style.dp
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
        text: b.label
        color: b.lit ? Style.text : Style.dim
        font.pixelSize: 11.5 * Style.dp
    }
    MouseArea {
        id: area
        anchors.fill: parent
        onClicked: b.clicked()
        onPressAndHold: b.pressAndHold()
    }
}
