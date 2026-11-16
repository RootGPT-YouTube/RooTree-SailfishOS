pragma Singleton
import QtQuick
// finto: la home del telefono e poco altro
QtObject {
    property string home: "/home/defaultuser"
    property bool rootActive: false
    property bool rootStarting: false
    property bool developerMode: true
    property bool rootHelperInstalled: true
    property var bookmarks: []
    function places() { return [{ kind: 0, label: "Memoria interna", path: home, total: 100, free: 50 }] }
    function ambience() { return ({}) }
    function exists(p) { return true }
    function isWritable(p) { return p.indexOf("/@remote/") !== 0 }
    function parentOf(p) { var i = p.lastIndexOf("/"); return i <= 0 ? "/" : p.substring(0, i) }
    function join(d, n) { return d + "/" + n }
    function isArchive(n) { return false }
    function archiveOf(p) { return "" }
    function trashSetDays(d) {}
    function trashPurge() {}
    function trashHint(l) { return "trash" }
    function isBookmarked(p) { return false }
    function sshKeys() { return [] }
    function sshConfigHosts() { return [] }
    function info(p) { return ({}) }
    function lastError() { return "" }
}
