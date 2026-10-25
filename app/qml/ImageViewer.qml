import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Il visore delle immagini: a tutto schermo, si scorre da una foto all'altra
// della cartella, due dita (o un doppio tocco) per ingrandire. Toccando la
// foto spariscono e ricompaiono le barre.
Popup {
    id: viewer
    property var images: []        // [{path, name, size, mtime}]
    property int current: 0
    property bool chrome: true
    signal openWithRequested(string path)
    signal propertiesRequested(string path)

    parent: Overlay.overlay
    x: 0; y: 0
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    padding: 0
    modal: true
    closePolicy: Popup.NoAutoClose
    background: Rectangle { color: "#000000" }

    function openAt(list, index) {
        images = list
        current = index
        chrome = true
        open()
        pages.positionViewAtIndex(index, ListView.Beginning)
        pages.currentIndex = index
    }

    readonly property var item: images[current] || ({})

    contentItem: Item {
        ListView {
            id: pages
            anchors.fill: parent
            orientation: ListView.Horizontal
            snapMode: ListView.SnapOneItem
            highlightRangeMode: ListView.StrictlyEnforceRange
            boundsBehavior: Flickable.StopAtBounds
            model: viewer.images
            cacheBuffer: width
            interactive: currentItem ? currentItem.zoom <= 1.01 : true
            onCurrentIndexChanged: viewer.current = currentIndex
            delegate: Flickable {
                id: page
                property real zoom: 1
                width: pages.width
                height: pages.height
                contentWidth: width * zoom
                contentHeight: height * zoom
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                function zoomAt(z, cx, cy) {
                    z = Math.max(1, Math.min(6, z))
                    var fx = (contentX + cx) / contentWidth, fy = (contentY + cy) / contentHeight
                    zoom = z
                    contentX = Math.max(0, fx * contentWidth - cx)
                    contentY = Math.max(0, fy * contentHeight - cy)
                }
                Image {
                    width: page.contentWidth
                    height: page.contentHeight
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    // dal fornitore «picture» di RooTree: legge anche WebP, TIFF e BMP
                    // (le immagini Qt del runtime no) e ruota secondo l'EXIF.
                    // Abbastanza per ingrandire, non tanto da riempire la memoria con 50 MP
                    sourceSize: Qt.size(Math.min(4096, page.width * 2.5), Math.min(4096, page.height * 2.5))
                    source: Math.abs(index - pages.currentIndex) <= 1 ? "image://picture/" + encodeURIComponent(modelData.path) : ""
                    BusyIndicator { anchors.centerIn: parent; running: parent.status === Image.Loading }
                    Text {
                        anchors.centerIn: parent
                        visible: parent.status === Image.Error
                        text: Style.t("Immagine non leggibile", "Cannot read the image")
                        color: Style.dim
                        font.pixelSize: Style.fontM
                    }
                }
                PinchArea {
                    anchors.fill: parent
                    property real startZoom: 1
                    onPinchStarted: startZoom = page.zoom
                    onPinchUpdated: function (p) { page.zoomAt(startZoom * p.scale, p.center.x - page.contentX, p.center.y - page.contentY) }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: viewer.chrome = !viewer.chrome
                        onDoubleClicked: function (m) { page.zoomAt(page.zoom > 1.01 ? 1 : 2.5, m.x - page.contentX, m.y - page.contentY) }
                    }
                }
            }
        }

        ViewerBar {
            anchors { left: parent.left; right: parent.right; top: parent.top }
            visible: viewer.chrome
            title: viewer.item.name || ""
            subtitle: (viewer.current + 1) + " / " + viewer.images.length + " · " + Style.size(viewer.item.size || 0) + " · " + Style.date(viewer.item.mtime)
            onCloseClicked: viewer.close()
            IconButton { icon: "share"; onClicked: Opener.share([viewer.item.path]) }
            IconButton { icon: "openwith"; onClicked: viewer.openWithRequested(viewer.item.path) }
            IconButton { icon: "info"; onClicked: viewer.propertiesRequested(viewer.item.path) }
        }
    }
}
