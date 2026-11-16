import QtQuick
// finto: ogni cartella contiene «Album», «Vacanze» e un file
ListModel {
    id: m
    property string path
    property bool showHidden: false
    property int sortKey: 2
    property bool descending: false
    property bool dirsFirst: true
    property string error: ""
    property int dirCount: 2
    property int fileCount: 1
    property real filesSize: 4
    property int selectedCount: 0
    property string archive: ""
    readonly property string remote: path.indexOf("/@remote/") === 0 ? path.split("/")[2] : ""
    property bool readOnly: false
    onPathChanged: fill()
    Component.onCompleted: fill()
    function fill() {
        clear()
        append({ name: "Album", path: path + "/Album", isDir: true, isLink: false, linkBroken: false, size: 0, mtime: 0, mode: 0, hidden: false, category: "folder", selected: false })
        append({ name: "Vacanze", path: path + "/Vacanze", isDir: true, isLink: false, linkBroken: false, size: 0, mtime: 0, mode: 0, hidden: false, category: "folder", selected: false })
        append({ name: "a.jpg", path: path + "/a.jpg", isDir: false, isLink: false, linkBroken: false, size: 4, mtime: 0, mode: 0, hidden: false, category: "image", selected: false })
    }
    function refresh() {}
    function setSorting(k, d, f) { sortKey = k; descending = d; dirsFirst = f }
    function indexOf(n) { return -1 }
    function setSelected(r, s) {}
    function toggleSelected(r) {}
    function selectAll() {}
    function clearSelection() {}
    function selectedPaths() { return [] }
    function coreVersion() { return "banco" }
}
