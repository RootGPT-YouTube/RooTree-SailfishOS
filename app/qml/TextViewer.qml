import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "."

// Il visore del testo: sola lettura, a capo automatico o no, i file troppo
// grandi si mostrano tagliati (con l'avviso), quelli binari no.
Popup {
    id: viewer
    property string path
    property var content: ({})
    property bool wrap: true
    signal openWithRequested(string path)

    parent: Overlay.overlay
    x: 0; y: 0
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    padding: 0
    modal: true
    closePolicy: Popup.NoAutoClose
    background: Rectangle { color: Style.bg }

    function openFor(p) {
        path = p
        content = Opener.readText(p)
        open()
        flick.contentY = 0
        flick.contentX = 0
    }

    contentItem: Item {
        ViewerBar {
            id: bar
            anchors { left: parent.left; right: parent.right; top: parent.top }
            title: viewer.path.split("/").pop()
            subtitle: viewer.content.truncated ? Style.t("Mostrati i primi 2 MB", "Showing the first 2 MB") : ""
            onCloseClicked: viewer.close()
            IconButton { icon: "wrap"; checked: viewer.wrap; onClicked: viewer.wrap = !viewer.wrap }
            IconButton { icon: "share"; onClicked: Opener.share([viewer.path]) }
            IconButton { icon: "openwith"; onClicked: viewer.openWithRequested(viewer.path) }
        }
        Flickable {
            id: flick
            anchors { left: parent.left; right: parent.right; top: bar.bottom; bottom: parent.bottom }
            contentWidth: viewer.wrap ? width : Math.max(width, body.implicitWidth + 2 * Style.margin)
            contentHeight: body.implicitHeight + 2 * Style.margin
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            visible: viewer.content.text !== undefined
            TextEdit {
                id: body
                x: Style.margin
                y: Style.margin
                width: viewer.wrap ? flick.width - 2 * Style.margin : implicitWidth
                readOnly: true
                selectByMouse: false
                textFormat: TextEdit.PlainText
                wrapMode: viewer.wrap ? TextEdit.WrapAtWordBoundaryOrAnywhere : TextEdit.NoWrap
                text: viewer.content.text || ""
                color: Style.text
                font.family: "monospace"
                font.pixelSize: 12.5 * Style.dp
            }
        }
        Column {
            visible: viewer.content.text === undefined
            anchors.centerIn: parent
            width: parent.width * 0.8
            spacing: 12 * Style.dp
            Text {
                font.family: Style.fontFamily
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: viewer.content.binary ? Style.t("Non è un file di testo.", "This is not a text file.")
                                            : (viewer.content.error || "")
                color: Style.text
                font.pixelSize: Style.fontL
            }
            SteelPill {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Style.t("Apri con…", "Open with…")
                onClicked: viewer.openWithRequested(viewer.path)
            }
        }
    }
}
