pragma Singleton
import QtQuick
QtObject {
    property string text: ""
    function readText(p) { return { text: text } }
    function share(l) {}
}
