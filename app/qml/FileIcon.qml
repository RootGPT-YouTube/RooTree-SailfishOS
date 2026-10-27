import QtQuick
import QtQuick.Effects
import "."

// L'icona di un elemento: la sagoma d'acciaio del tipo e, per immagini, video
// e PDF, l'anteprima vera (quando c'è) in una cornice d'acciaio, con la piccola
// icona del tipo nell'angolo, come gli overlay di Dolphin. Nel tema Sailfish
// le sagome si ricolorano (cartelle del colore di evidenziazione, file di
// quello secondario, per distinguerli) e la cornice è un filo sottile.
Item {
    id: fi
    property string path
    property string category
    property bool thumbnails: true
    property bool small: false

    readonly property bool wantsThumb: thumbnails && (category === "image" || category === "video" || category === "pdf")
    readonly property bool hasThumb: thumb.status === Image.Ready
    readonly property color tint: category.indexOf("folder") === 0 ? Style.sfHighlight : Style.sfSecondaryHighlight
    component Tinted: MultiEffect { colorization: 1 }

    Image {
        anchors.fill: parent
        visible: !fi.hasThumb
        source: "icons/tipi/" + fi.category + ".png"
        sourceSize: Qt.size(width, height)
        smooth: true
        layer.enabled: Style.sailfish
        layer.effect: Tinted { colorizationColor: fi.tint }
    }
    // la cornice d'acciaio, grande quanto l'anteprima
    Rectangle {
        visible: fi.hasThumb
        anchors.centerIn: parent
        width: thumb.paintedWidth + 2 * border.width
        height: thumb.paintedHeight + 2 * border.width
        radius: 3 * Style.dp
        color: Style.sailfish ? "transparent" : "#20262d"
        border.width: (Style.sailfish ? 1 : (fi.small ? 1.5 : 2.5)) * Style.dp
        border.color: Style.sailfish ? Style.alpha(Style.sfHighlight, 0.5) : "#8a97a5"
        Rectangle {
            visible: !Style.sailfish
            anchors.fill: parent
            anchors.margins: parent.border.width * 0.4
            radius: parent.radius
            color: "transparent"
            border.width: 1
            border.color: "#40ffffff"
        }
    }
    Image {
        id: thumb
        anchors.fill: parent
        anchors.margins: (fi.small ? 2 : 4) * Style.dp
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        cache: false
        source: fi.wantsThumb ? "image://thumb/" + encodeURIComponent(fi.path) : ""
        sourceSize: Qt.size(fi.small ? 128 : 256, fi.small ? 128 : 256)
    }
    Image {
        visible: fi.hasThumb && !fi.small
        anchors { right: parent.right; bottom: parent.bottom; rightMargin: -2 * Style.dp; bottomMargin: -2 * Style.dp }
        width: parent.width * 0.34
        height: width
        source: "icons/tipi/" + fi.category + ".png"
        sourceSize: Qt.size(width, height)
        layer.enabled: Style.sailfish
        layer.effect: Tinted { colorizationColor: fi.tint }
    }
}
