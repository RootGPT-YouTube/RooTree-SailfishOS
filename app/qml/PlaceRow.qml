import QtQuick
import "."

// Una memoria del menù a tendina con, sotto, le sue cartelle predefinite
// (si aprono con la freccia della riga). `owner` è il PlacesMenu.
Column {
    id: pr
    property var owner
    property var place: null
    property string icon
    property string text
    property int depth: 0
    readonly property var dirs: place ? pr.owner.dirsOf(place.path) : []
    readonly property bool open: place !== null && pr.owner.isOpen(place.path)
    width: parent ? parent.width : 0
    visible: place !== null
    MenuRow {
        depth: pr.depth
        icon: pr.icon
        text: pr.text
        detail: pr.place ? pr.owner.freeText(pr.place) : ""
        expandable: pr.dirs.length > 0
        expanded: pr.open
        onClicked: pr.owner.go(pr.place.path)
        onToggled: pr.owner.toggle(pr.place.path)
        infoButton: true
        onInfoClicked: pr.owner.info(pr.place)
    }
    Repeater {
        model: pr.open ? pr.dirs : []
        delegate: MenuRow {
            depth: pr.depth + 1
            image: modelData.image
            text: modelData.name
            onClicked: pr.owner.go(modelData.path)
        }
    }
}
