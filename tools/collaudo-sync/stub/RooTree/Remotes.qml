pragma Singleton
import QtQuick
// finto: due connessioni (NAS in SFTP con password nella cassaforte, kDrive)
QtObject {
    id: r
    property var list: [
        { id: "nas", type: "sftp", name: "NAS Remoto", host: "nas", user: "u", auth: "password", path: "/media/nas" },
        { id: "kd", type: "kdrive", name: "kDrive", auth: "token" }
    ]
    property var types: [
        { type: "sftp", label: "SFTP (SSH)", group: "server", icon: "ssh", needsServer: true, auths: ["password"] },
        { type: "kdrive", label: "kDrive (Infomaniak)", group: "cloud", icon: "kdrive", needsServer: false, auths: ["token"] },
        { type: "mega", label: "Mega", group: "cloud", icon: "mega", needsServer: false, auths: ["password"] }
    ]
    property bool vaultExists: true
    property bool vaultUnlocked: false
    property var connectedIds: []
    property var connectCalls: []
    // l'esito del prossimo collegamento ("" = riuscito, "NET"…)
    property string nextCode: ""
    signal connectFinished(string id, string code, string message, string path)
    signal changed()
    signal vaultChanged()
    signal authUrl(string id, string url)
    signal authFinished(string id, string code, string message)
    signal megaRevoked(string id, string error)
    signal trashDone(string id, string action, string error, string json)
    function typeInfo(t) { for (var i = 0; i < types.length; ++i) if (types[i].type === t) return types[i]; return ({}) }
    function get(id) { for (var i = 0; i < list.length; ++i) if (list[i].id === id) return list[i]; return ({}) }
    function describe(p) { return p.host ? (p.user + "@" + p.host) : (typeInfo(p.type).label || "") }
    function isConnected(id) { return connectedIds.indexOf(id) >= 0 }
    function connecting(id) { return false }
    function hasSecret(id) { return vaultUnlocked }
    function entryPath(id) { return isConnected(id) ? "/@remote/" + id + (get(id).path || "") : "" }
    function vaultUnlock(m) { if (m !== "giusta") return "WRONG"; vaultUnlocked = true; vaultChanged(); return "" }
    function connect(id) {
        connectCalls = connectCalls.concat([id])
        var code = nextCode
        Qt.callLater(function () {
            if (code === "") r.connectedIds = r.connectedIds.concat([id])
            r.connectFinished(id, code, code === "NET" ? "timeout" : "", "/@remote/" + id + (r.get(id).path || ""))
        })
    }
}
