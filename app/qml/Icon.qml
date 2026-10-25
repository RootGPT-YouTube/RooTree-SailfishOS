import QtQuick
import "."

// Un'icona SVG di icons/ (Font Awesome, già colorata), quadrata.
Image {
    property string name
    property real size: Style.icon
    width: size
    height: size
    source: name ? Qt.resolvedUrl("icons/" + name + ".svg") : ""
    sourceSize: Qt.size(size, size)
    fillMode: Image.PreserveAspectFit
    smooth: true
}
