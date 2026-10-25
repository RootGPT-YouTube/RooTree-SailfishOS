import QtQuick
import QtQuick.Window
import RooTree

// Prima finestra di prova (Fase 0): elenco della cartella dal nucleo Rust.
Window {
    id: win
    visible: true
    color: "#14191f"
    title: "RooTree"

    readonly property real u: Math.max(1, Screen.pixelDensity / 4)   // ~1 mm / 4

    FolderModel {
        id: folder
        path: homePath
    }

    Rectangle {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: 18 * win.u
        color: "#1f2730"
        Text {
            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; margins: 4 * win.u }
            text: folder.path
            color: "#e6edf3"
            font.pixelSize: 5 * win.u
            elide: Text.ElideLeft
        }
        MouseArea { anchors.fill: parent; onClicked: folder.cdUp() }
        Rectangle { anchors { left: parent.left; right: parent.right; bottom: parent.bottom } height: 2; color: "#0073ff" }
    }

    ListView {
        anchors { left: parent.left; right: parent.right; top: header.bottom; bottom: footer.top }
        clip: true
        model: folder
        delegate: Rectangle {
            width: ListView.view.width
            height: 14 * win.u
            color: pressArea.pressed ? "#26313c" : "transparent"
            Text {
                anchors { left: parent.left; right: sizeText.left; verticalCenter: parent.verticalCenter; leftMargin: 4 * win.u }
                text: (isDir ? "▸ " : "   ") + name
                color: isDir ? "#7fb6ff" : "#e6edf3"
                font.pixelSize: 4.5 * win.u
                elide: Text.ElideRight
            }
            Text {
                id: sizeText
                anchors { right: parent.right; verticalCenter: parent.verticalCenter; rightMargin: 4 * win.u }
                text: isDir ? "" : size + " B"
                color: "#8b98a5"
                font.pixelSize: 3.5 * win.u
            }
            MouseArea { id: pressArea; anchors.fill: parent; onClicked: folder.open(index) }
        }
    }

    Text {
        anchors.centerIn: parent
        visible: folder.error !== ""
        width: parent.width * 0.8
        wrapMode: Text.Wrap
        horizontalAlignment: Text.AlignHCenter
        text: folder.error
        color: "#ff6b6b"
        font.pixelSize: 4 * win.u
    }

    Text {
        id: footer
        anchors { left: parent.left; bottom: parent.bottom; margins: 3 * win.u }
        text: "rootree-core " + folder.coreVersion() + " · " + folder.count + " elementi"
        color: "#8b98a5"
        font.pixelSize: 3.5 * win.u
    }
}
