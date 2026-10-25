import QtQuick
import "."

// L'anello LED blu elettrico intorno a un disco: una riga brillante con due
// più deboli intorno per l'alone (come i pulsanti accesi di RooTundra).
Item {
    id: ring
    property real diameter: width
    property color color: Qt.rgba(0.0, 0.45, 1.0, 1)
    Repeater {
        model: [{ w: 0.075, a: 0.22 }, { w: 0.045, a: 0.45 }, { w: 0.018, a: 1.0 }]
        Rectangle {
            anchors.centerIn: parent
            readonly property real line: Math.max(1, Math.round(ring.diameter * modelData.w))
            width: ring.diameter * 0.94 + line - Math.max(1, Math.round(ring.diameter * 0.018))
            height: width
            radius: width / 2
            color: "transparent"
            border.width: line
            border.color: Qt.rgba(ring.color.r, ring.color.g, ring.color.b, modelData.a)
        }
    }
}
