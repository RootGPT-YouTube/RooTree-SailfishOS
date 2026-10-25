// Da RooTundra (appdrawer/FrostCorner.qml, BSD), per i pannelli di ghiaccio di
// RooTree. fleur 12/10: frost on a right corner of the ice frame of the app drawer:
// thin branching cracks, sparkles and a white haze, drawn only on the band of
// the frame (between the outer and the inner rounded edge). Drawn once, the
// same every time (fixed seed). The corner is the top right one, or the bottom
// right one when flipped.

import QtQuick

Canvas {
    id: frost

    property real outerRadius
    property real frame
    property int seed: 1
    property bool flipped: false

    width: outerRadius * 6
    height: outerRadius * 6

    onWidthChanged: requestPaint()
    onFrameChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d")
        ctx.reset()

        var w = width, h = height
        var r = outerRadius, f = frame
        var s = seed

        // the same pseudo random numbers at every paint
        function rnd() {
            s = (s * 16807) % 2147483647
            return (s - 1) / 2147483646
        }

        if (flipped) {
            ctx.translate(0, h)
            ctx.scale(1, -1)
        }

        // only the band of the frame (inner radius as in AppDrawer.qml)
        ctx.fillRule = Qt.OddEvenFill
        ctx.beginPath()
        ctx.roundedRect(-r, 0, w + r, h + r, r, r)
        ctx.roundedRect(-r, f, w + r - f, h + r, r - f * 0.6, r - f * 0.6)
        ctx.clip()

        // white haze, thicker in the corner
        var haze = ctx.createRadialGradient(w, 0, 0, w, 0, w * 0.9)
        haze.addColorStop(0.0, "rgba(255,255,255,0.55)")
        haze.addColorStop(0.5, "rgba(235,248,255,0.18)")
        haze.addColorStop(1.0, "rgba(235,248,255,0)")
        ctx.fillStyle = haze
        ctx.fillRect(0, 0, w, h)

        // a crack: a broken line with a few branches
        function crack(x, y, angle, length, depth) {
            ctx.beginPath()
            ctx.moveTo(x, y)
            var steps = 4 + Math.floor(rnd() * 3)
            var step = length / steps
            for (var i = 0; i < steps; i++) {
                angle += (rnd() - 0.5) * 0.7
                x += Math.cos(angle) * step
                y += Math.sin(angle) * step
                ctx.lineTo(x, y)
                if (depth > 0 && rnd() < 0.45) {
                    ctx.stroke()
                    crack(x, y, angle + (rnd() < 0.5 ? -1 : 1) * (0.5 + rnd() * 0.6),
                          length * 0.45, depth - 1)
                    ctx.beginPath()
                    ctx.moveTo(x, y)
                }
            }
            ctx.stroke()
        }

        ctx.lineCap = "round"
        ctx.lineJoin = "round"
        ctx.strokeStyle = "rgba(255,255,255,0.75)"
        ctx.lineWidth = Math.max(1.5, f * 0.07)
        // along the top band (going left) and down the right band
        for (var a = 0; a < 3; a++)
            crack(w - f * (0.3 + rnd() * 0.4), f * (0.2 + rnd() * 0.5), Math.PI + (rnd() - 0.5) * 0.4, w * (0.45 + rnd() * 0.4), 2)
        for (var b = 0; b < 3; b++)
            crack(w - f * (0.2 + rnd() * 0.5), f * (0.3 + rnd() * 0.4), Math.PI / 2 + (rnd() - 0.5) * 0.4, h * (0.45 + rnd() * 0.4), 2)
        // around the bend of the corner
        for (var c = 0; c < 3; c++)
            crack(w - r * 0.3, r * 0.3, Math.PI * (0.55 + rnd() * 0.4), r * (1 + rnd()), 1)

        // sparkles: little four pointed stars
        function sparkle(x, y, size) {
            ctx.beginPath()
            ctx.moveTo(x - size, y)
            ctx.lineTo(x + size, y)
            ctx.moveTo(x, y - size)
            ctx.lineTo(x, y + size)
            ctx.stroke()
            ctx.beginPath()
            ctx.arc(x, y, size * 0.25, 0, Math.PI * 2)
            ctx.fill()
        }
        ctx.strokeStyle = "rgba(255,255,255,0.7)"
        ctx.fillStyle = "rgba(255,255,255,0.95)"
        ctx.lineWidth = 1
        for (var k = 0; k < 5; k++) {
            if (rnd() < 0.5)
                sparkle(w - f * (0.3 + rnd() * 0.4), f + rnd() * h * 0.8, f * (0.12 + rnd() * 0.1))
            else
                sparkle(f + rnd() * w * 0.8, f * (0.3 + rnd() * 0.4), f * (0.12 + rnd() * 0.1))
        }
    }
}
