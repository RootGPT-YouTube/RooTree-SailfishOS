import QtQuick
import QtQuick.Effects
import "."

// Un'icona SVG di icons/ (Font Awesome, già colorata), quadrata. Nel tema
// Sailfish si ricolora tutta con `tint` (il colore del testo dell'ambience,
// o quello di evidenziazione quando è accesa).
Image {
    id: ic
    property string name
    property real size: Style.icon
    property color tint: Style.iconTint
    width: size
    height: size
    source: name ? Qt.resolvedUrl("icons/" + name + ".svg") : ""
    sourceSize: Qt.size(size, size)
    fillMode: Image.PreserveAspectFit
    smooth: true
    layer.enabled: Style.sailfish && ic.tint.a > 0
    layer.effect: MultiEffect {
        colorization: 1
        colorizationColor: ic.tint
    }
}
