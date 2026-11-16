import QtQuick
ListModel {
    property bool running: false
    property int found: 0
    property int scanned: 0
    property string current: ""
    property string error: ""
    property bool limited: false
    function start(a, b, c, d, e) {}
    function stop() {}
}
