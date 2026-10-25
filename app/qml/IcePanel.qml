import QtQuick
import "."

// Una lastra di ghiaccio come il cassetto delle app di RooTundra (IceDrawer):
// cornice di vetro smerigliato con i riflessi in diagonale, la brina negli
// angoli, e dentro vetro scuro dove va il contenuto (`content`). Il vetro
// interno qui è quasi pieno: sotto non c'è lo sfondo sfocato ma l'app.
Item {
    id: ice
    default property alias content: holder.data
    property real frame: 12 * Style.dp
    property real radius: 18 * Style.dp

    Canvas {
        id: glass
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            var g = ctx.createLinearGradient(0, 0, width, height)
            g.addColorStop(0.0, "rgba(244,252,255,0.85)")
            g.addColorStop(0.2, "rgba(169,221,245,0.65)")
            g.addColorStop(0.42, "rgba(232,248,255,0.80)")
            g.addColorStop(0.6, "rgba(140,198,230,0.60)")
            g.addColorStop(0.8, "rgba(214,241,255,0.76)")
            g.addColorStop(1.0, "rgba(160,210,238,0.65)")
            ctx.fillStyle = g
            ctx.beginPath()
            ctx.roundedRect(0, 0, width, height, ice.radius, ice.radius)
            ctx.fill()
        }
    }
    // spessore del ghiaccio: luce sulla metà esterna della cornice, ombra su quella interna
    Rectangle {
        anchors.fill: parent
        anchors.margins: Math.round(ice.frame * 0.2)
        radius: ice.radius - ice.frame * 0.2
        color: "transparent"
        border.width: Math.round(ice.frame * 0.25)
        border.color: "#59FFFFFF"
    }
    Rectangle {
        anchors.fill: parent
        anchors.margins: Math.round(ice.frame * 0.62)
        radius: ice.radius - ice.frame * 0.5
        color: "transparent"
        border.width: Math.round(ice.frame * 0.3)
        border.color: "#334A7FA3"
    }
    Rectangle {
        anchors.fill: parent
        radius: ice.radius
        color: "transparent"
        border.width: Math.max(1, Math.round(1.2 * Style.dp))
        border.color: "#F0FFFFFF"
    }
    FrostCorner {
        id: frostTop
        x: parent.width - width
        anchors.top: parent.top
        outerRadius: ice.radius
        frame: ice.frame
        seed: 7
    }
    FrostCorner {
        id: frostLeft
        x: 0
        anchors.bottom: parent.bottom
        outerRadius: ice.radius
        frame: ice.frame
        seed: 23
        flipped: true
        transform: Scale { origin.x: frostLeft.width / 2; xScale: -1 }
    }
    // dentro la cornice: vetro scuro, la scritta bianca resta leggibile
    Rectangle {
        id: inner
        anchors.fill: parent
        anchors.margins: ice.frame
        radius: ice.radius - ice.frame * 0.6
        border.width: 1
        border.color: "#8CFFFFFF"
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#F2183A50" }
            GradientStop { position: 1.0; color: "#F70B2233" }
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: 1
            radius: parent.radius - 1
            color: "transparent"
            border.width: Math.max(1, Math.round(1.5 * Style.dp))
            border.color: "#40001424"
        }
        Rectangle {
            anchors { left: parent.left; right: parent.right; top: parent.top; margins: 2 }
            height: Math.min(parent.height * 0.18, 60 * Style.dp)
            radius: parent.radius
            gradient: Gradient {
                GradientStop { position: 0.0; color: "#2EFFFFFF" }
                GradientStop { position: 1.0; color: "#00FFFFFF" }
            }
        }
        Item {
            id: holder
            anchors.fill: parent
            anchors.margins: 4 * Style.dp
            clip: true
        }
    }
}
