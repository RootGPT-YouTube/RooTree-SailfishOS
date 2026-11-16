pragma Singleton
import QtQuick
// finto: tiene le sincronizzazioni in memoria e annota i giri chiesti
QtObject {
    id: s
    property var list: []
    property var started: []
    property var results: ({})
    property int nextJob: 100
    signal changed()
    signal wanted(string id)
    function get(id) { for (var i = 0; i < list.length; ++i) if (list[i].id === id) return list[i]; return ({}) }
    function save(m) {
        var c = Object.assign({}, m)
        if (!c.id) c.id = "s" + (list.length + 1)
        var l = list.filter(function (x) { return x.id !== c.id }); l.push(c); list = l
        return c.id
    }
    function remove(id) { list = list.filter(function (x) { return x.id !== id }) }
    function running(id) { return false }
    function start(id, confirmed) { started = started.concat([{ id: id, confirmed: confirmed }]); return nextJob++ }
    function setResult(id, text, error) {
        var r = Object.assign({}, results); r[id] = { text: text, error: error }; results = r
        list = list.map(function (x) { if (x.id !== id) return x; var c = Object.assign({}, x); c.lastResult = text; c.lastError = error; c.lastRun = Date.now(); return c })
    }
    function forRemote(rid) { return list.filter(function (x) { return x.remote === rid }) }
}
