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
    // una chiavetta o una scheda inserita ma non montata: il tocco la monta
    readonly property bool unmounted: place !== null && place.mounted === false
    readonly property bool removable: place !== null && (place.kind === 2 || place.kind === 3)
    readonly property var dirs: place && !unmounted ? pr.owner.dirsOf(place.path) : []
    readonly property bool open: place !== null && pr.owner.isOpen(place.path)
    width: parent ? parent.width : 0
    visible: place !== null
    MenuRow {
        depth: pr.depth
        icon: pr.icon
        text: pr.text
        detail: !pr.place ? "" : pr.unmounted
                ? Style.t("Tocca per montare", "Tap to mount") + (pr.place.total > 0 ? " · " + Style.size(pr.place.total) : "")
                : pr.owner.freeText(pr.place)
        // si apre sempre: sotto ci sono almeno il cestino della memoria
        expandable: pr.place !== null && !pr.unmounted
        expanded: pr.open
        reserveToggle: pr.unmounted
        onClicked: pr.unmounted ? pr.owner.mount(pr.place) : pr.owner.go(pr.place.path)
        onToggled: pr.owner.toggle(pr.place.path)
        infoButton: !pr.unmounted
        onInfoClicked: pr.owner.info(pr.place)
        ejectButton: pr.removable && !pr.unmounted
        onEjectClicked: pr.owner.eject(pr.place)
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
    // il cestino di questa memoria, insieme alle sue cartelle
    MenuRow {
        visible: pr.open
        depth: pr.depth + 1
        icon: "trash"
        iconSize: 24
        text: Style.t("Cestino", "Trash")
        onClicked: pr.owner.trash(pr.place, pr.text)
    }
}
