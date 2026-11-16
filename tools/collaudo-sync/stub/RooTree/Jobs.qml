pragma Singleton
import QtQuick
// finto: l'elenco dei lavori e il segnale di fine si comandano dal banco
QtObject {
    property var jobs: []
    property int activeCount: 0
    property var question: ({})
    property var clipboard: []
    property bool clipboardCut: false
    property var statusMap: ({})
    property var forgotten: []
    signal finished(int id, int kind, int state, string error)
    function status(id) { return statusMap[id] || ({}) }
    function forget(id) { forgotten = forgotten.concat([id]) }
    function forgetFinished() {}
    // dal banco: un lavoro di sincronizzazione finito con questo esito
    function fakeFinish(id, state, info, error) {
        var m = Object.assign({}, statusMap); m[id] = { id: id, kind: 9, state: state, info: info }; statusMap = m
        finished(id, 9, state, error || "")
    }
}
