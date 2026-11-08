import QtQuick
import "."

// Due maniglie per scegliere col dito quale parte del testo selezionare, sul
// campo `field` (un TextField): su touch il trascinamento sul testo sposta il
// cursore, non seleziona. Compaiono quando c'è una selezione; trascinando una
// maniglia si sposta quel capo. Va messo come fratello del campo, sopra di lui.
Item {
    id: h
    property Item field
    anchors.fill: parent
    z: 5

    readonly property bool active: field && field.selectedText.length > 0
    readonly property real knob: 22 * Style.dp

    // la posizione (nelle coordinate di h) di un capo della selezione
    function at(pos) {
        if (!field) return Qt.point(0, 0)
        var r = field.positionToRectangle(pos)
        return field.mapToItem(h, r.x, r.y + r.height)
    }
    // il carattere sotto un punto di h
    function posAt(x, y) {
        var p = h.mapToItem(field, x, y)
        return field.positionAt(p.x, Math.min(Math.max(p.y, 1), field.height - 1))
    }

    component Handle: Item {
        id: one
        property bool start: true
        readonly property point tip: h.at(start ? field.selectionStart : field.selectionEnd)
        visible: h.active
        width: h.knob * 2
        height: h.knob * 2
        x: tip.x - width / 2
        y: tip.y - 4 * Style.dp
        // un filo dal testo e un disco blu, come le maniglie di Sailfish
        Rectangle { anchors.horizontalCenter: parent.horizontalCenter; y: 0; width: 2 * Style.dp; height: h.knob * 0.6; color: Style.accent }
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            y: h.knob * 0.5
            width: h.knob; height: width; radius: width / 2
            color: Style.accent
            border.color: Style.text
            border.width: 1.5 * Style.dp
        }
        MouseArea {
            anchors.fill: parent
            preventStealing: true
            onPositionChanged: function (m) {
                var p = mapToItem(h, m.x, -h.knob * 0.8)
                var pos = h.posAt(p.x, p.y)
                var s = field.selectionStart, e = field.selectionEnd
                if (one.start) {
                    if (pos < e) field.select(pos, e)
                } else {
                    if (pos > s) field.select(s, pos)
                }
            }
        }
    }
    Handle { start: true }
    Handle { start: false }
}
