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
    // un tocco su una maniglia senza trascinarla: per chi sta sotto è un tocco
    // sul testo (la maniglia copre la riga sotto la selezione), coordinate di h
    signal tapped(real x, real y)
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
            property bool moved: false
            property point down
            // in coordinate della scena: la maniglia si sposta mentre la si
            // trascina, quindi le coordinate locali non bastano
            property point fingerAt
            property point tipAt
            onPressed: function (m) {
                moved = false
                down = Qt.point(m.x, m.y)
                fingerAt = mapToItem(null, m.x, m.y)
                tipAt = h.mapToItem(null, one.tip.x, one.tip.y)
            }
            onReleased: function (m) {
                if (!moved) {
                    var p = mapToItem(h, m.x, m.y)
                    h.tapped(p.x, p.y)
                }
            }
            onPositionChanged: function (m) {
                if (Math.abs(m.x - down.x) + Math.abs(m.y - down.y) > 8 * Style.dp) moved = true
                if (!moved) return
                // ⛔ prima contava solo lo spostamento orizzontale (andava bene
                // per Rinomina, una riga sola): nell'editor non si scendeva
                // mai alla riga sotto (12/11). Ora: la punta di partenza più
                // lo spostamento del dito, un filo sopra il fondo della riga
                var f = mapToItem(null, m.x, m.y)
                var p = h.mapFromItem(null, tipAt.x + f.x - fingerAt.x, tipAt.y + f.y - fingerAt.y - 3 * Style.dp)
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
