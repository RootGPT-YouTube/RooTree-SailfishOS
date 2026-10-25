import QtQuick
import "."

// Il percorso «a briciole», come in Dolphin: la memoria, poi una casella per
// ogni cartella. Si tocca una casella per saltarci. Scorre di lato se è lungo,
// e resta sempre agganciato alla fine.
Flickable {
    id: bar
    property string path
    property string rootPath: "/"
    property string rootLabel: "/"
    signal navigate(string path)

    contentWidth: row.width
    contentHeight: height
    clip: true
    flickableDirection: Flickable.HorizontalFlick
    boundsBehavior: Flickable.StopAtBounds
    onContentWidthChanged: contentX = Math.max(0, contentWidth - width)
    onWidthChanged: contentX = Math.max(0, contentWidth - width)

    // [{label, path}]
    readonly property var crumbs: {
        var out = [{ label: rootLabel, path: rootPath }]
        var rest = path.substring(rootPath.length)
        var acc = rootPath === "/" ? "" : rootPath
        var parts = rest.split("/")
        for (var i = 0; i < parts.length; ++i) {
            if (parts[i] === "")
                continue
            acc += "/" + parts[i]
            out.push({ label: parts[i], path: acc })
        }
        return out
    }

    Row {
        id: row
        height: bar.height
        Repeater {
            model: bar.crumbs
            delegate: Row {
                height: row.height
                Icon {
                    visible: index > 0
                    anchors.verticalCenter: parent.verticalCenter
                    name: "next"
                    size: 11 * Style.dp
                    opacity: 0.5
                }
                Rectangle {
                    readonly property bool last: index === bar.crumbs.length - 1
                    anchors.verticalCenter: parent.verticalCenter
                    height: parent.height - 12 * Style.dp
                    width: label.implicitWidth + 16 * Style.dp
                    radius: 6 * Style.dp
                    color: area.pressed ? Style.pressed : "transparent"
                    // la cartella dove si è: il filo LED sotto
                    Rectangle {
                        visible: parent.last
                        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 6 * Style.dp; bottomMargin: 2 * Style.dp }
                        height: Math.max(2, 2.2 * Style.dp)
                        radius: height / 2
                        color: Style.accent
                    }
                    Text {
                        id: label
                        anchors.centerIn: parent
                        text: modelData.label
                        color: parent.last ? Style.text : Style.dim
                        font.pixelSize: Style.fontM
                        font.bold: parent.last
                    }
                    MouseArea { id: area; anchors.fill: parent; onClicked: bar.navigate(modelData.path) }
                }
            }
        }
    }
}
