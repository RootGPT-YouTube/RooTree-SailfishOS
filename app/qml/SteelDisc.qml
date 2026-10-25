import QtQuick

// Da RooTundra (controlcenter/SteelDisc.qml, BSD): la faccia di un pulsante
// «acciaio / tech», con la cornice d'acciaio scuro, la faccia d'acciaio
// spazzolato in cerchio e sei piste del circuito incise. Si disegna una volta;
// anello LED e simbolo sono sopra (IconButton.qml).
Canvas {
    id: disc

    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d")
        ctx.reset()
        var c = width / 2
        var r = Math.min(width, height) / 2
        var ri = r * 0.88

        // cornice: acciaio scuro, più chiaro in alto
        var bz = ctx.createLinearGradient(0, c - r, 0, c + r)
        bz.addColorStop(0, "rgb(92,102,112)")
        bz.addColorStop(1, "rgb(24,29,35)")
        ctx.fillStyle = bz
        ctx.beginPath()
        ctx.arc(c, c, r, 0, Math.PI * 2)
        ctx.fill()

        // faccia spazzolata: un gradiente conico è una spazzolatura girata in cerchio
        var cg = ctx.createConicalGradient(c, c, Math.PI / 6)
        var stops = [[0, 175], [0.12, 230], [0.25, 150], [0.4, 215], [0.55, 140],
                     [0.7, 225], [0.85, 160], [1, 175]]
        for (var i = 0; i < stops.length; i++) {
            var v = stops[i][1]
            cg.addColorStop(stops[i][0], "rgb(" + v + "," + (v + 6) + "," + (v + 12) + ")")
        }
        ctx.fillStyle = cg
        ctx.beginPath()
        ctx.arc(c, c, ri, 0, Math.PI * 2)
        ctx.fill()

        // un'ombra leggera verso il basso: la faccia è un po' bombata
        var sh = ctx.createLinearGradient(0, c - ri, 0, c + ri)
        sh.addColorStop(0, "rgba(255,255,255,0.10)")
        sh.addColorStop(0.55, "rgba(255,255,255,0)")
        sh.addColorStop(1, "rgba(0,10,20,0.22)")
        ctx.fillStyle = sh
        ctx.beginPath()
        ctx.arc(c, c, ri, 0, Math.PI * 2)
        ctx.fill()

        // piste incise: una riga scura con un filo chiaro sotto
        function traces(colour, dy) {
            ctx.strokeStyle = colour
            ctx.fillStyle = colour
            ctx.lineWidth = Math.max(1, r * 0.022)
            for (var k = 0; k < 6; k++) {
                var a = k * Math.PI / 3 + 0.35
                var x0 = c + Math.cos(a) * ri * 0.64, y0 = c + Math.sin(a) * ri * 0.64 + dy
                var x1 = c + Math.cos(a) * ri * 0.86, y1 = c + Math.sin(a) * ri * 0.86 + dy
                ctx.beginPath()
                ctx.moveTo(x0, y0)
                ctx.lineTo(x1, y1)
                ctx.stroke()
                ctx.beginPath()
                ctx.arc(x1, y1, Math.max(1.5, r * 0.04), 0, Math.PI * 2)
                ctx.fill()
            }
        }
        traces("rgba(255,255,255,0.35)", Math.max(1, r * 0.015))
        traces("rgba(55,65,75,0.75)", 0)
    }
}
