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
    // come il vero: sul disco solo la home; i server li dice Remotes (collegato = c'è).
    // Mai più «sempre sì»: nascose il ripiego sulla home del 16/11
    function exists(p) {
        if (p.indexOf("/@remote/") === 0) { var id = p.split("/")[2]; return Remotes.isConnected(id) }
        return p === home || p.indexOf(home + "/") === 0
    }
    // come il vero: su un server decide il server (sì), sul telefono la home
    function isWritable(p) { return p.indexOf("/@remote/") === 0 || p === home || p.indexOf(home + "/") === 0 }
    function parentOf(p) { var i = p.lastIndexOf("/"); return i <= 0 ? "/" : p.substring(0, i) }
    function join(d, n) { return d + "/" + n }
    property var madeDirs: []
    function makeDir(parent, name) { var a = madeDirs.slice(); a.push(parent + "/" + name); madeDirs = a; return "" }
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
