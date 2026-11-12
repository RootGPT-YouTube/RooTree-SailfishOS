import QtQuick
import QtQuick.Controls.Basic
import RooTree
import "app"
ApplicationWindow {
    id: w
    width: 400; height: 700; visible: true

    TextViewer { id: v; objectName: "viewer" }
    Component.onCompleted: console.log("DBG harness")
    function body() { return v.contentItem.children[1].contentItem.children[0] }
    // il punto (nella finestra) a metà del carattere i
    function pointOf(i) {
        var b = findBody()
        var r = b.positionToRectangle(i)
        var p = b.mapToItem(null, r.x + 2, r.y + r.height / 2)
        return { x: p.x, y: p.y }
    }
    function findBody() { return find(v.contentItem, "body") }
    function find(it, name) {
        if (!it) return null
        if (it.objectName === name) return it
        var kids = it.children || []
        for (var i = 0; i < kids.length; ++i) { var r = find(kids[i], name); if (r) return r }
        if (it.contentItem && it.contentItem !== it) return find(it.contentItem, name)
        return null
    }
    function state() {
        var b = findBody(), f = find(v.contentItem, "flick")
        return { lines: b.lineCount, bh: b.height, bw: b.width, fh: f.height, ch: f.contentHeight, cursor: b.cursorPosition, sel: b.selectedText, focus: b.activeFocus, text: b.text, contentY: f.contentY, editing: v.editing }
    }
}
