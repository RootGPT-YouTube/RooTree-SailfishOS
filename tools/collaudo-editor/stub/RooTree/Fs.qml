pragma Singleton
import QtQuick
QtObject {
    property bool rootActive: false
    function isWritable(p) { return true }
    function writeText(p, t) { return { error: "", backup: "" } }
    function ambience() { return ({}) }
}
