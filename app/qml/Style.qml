pragma Singleton
import QtQuick

// Colori, misure e testi comuni. `dp` lo imposta Main.qml dalla larghezza
// della finestra: 1 dp = 1/400 del lato corto (2,7 px sul POCO).
QtObject {
    property real dp: 2.7

    readonly property bool italiano: Qt.locale().name.substring(0, 2) === "it"
    function t(it, en) { return italiano ? it : en }

    // i colori di RooTundra (tastierino della schermata di blocco): acciaio
    // scuro, testo bianco ghiaccio, secondario azzurro ghiaccio, LED blu elettrico
    readonly property color bg: "#14181d"
    readonly property color bg2: "#20262d"
    readonly property color panel: "#2a3139"
    readonly property color panel2: "#3a434d"
    readonly property color line: "#3a434d"
    readonly property color steelBorder: "#6b7b8c"
    readonly property color text: "#eef6ff"
    readonly property color dim: "#a9c4e0"
    readonly property color accent: "#1a7dff"
    readonly property color accentSoft: "#401a7dff"
    readonly property color selected: "#331a7dff"
    readonly property color pressed: "#14ffffff"
    readonly property color danger: "#ff5a5a"
    readonly property color ok: "#5ccf8a"

    // Rajdhani è un carattere stretto: misure un po' più grandi del solito
    readonly property real fontS: 13.5 * dp
    readonly property real fontM: 16 * dp
    readonly property real fontL: 19.5 * dp
    readonly property real bar: 52 * dp
    readonly property real row: 54 * dp
    readonly property real icon: 24 * dp
    readonly property real margin: 12 * dp

    function size(bytes) {
        var u = italiano ? ["byte", "kB", "MB", "GB", "TB"] : ["bytes", "kB", "MB", "GB", "TB"]
        var i = 0
        var v = bytes
        while (v >= 1000 && i < u.length - 1) { v /= 1000; ++i }
        return (i === 0 ? v : Number(v).toLocaleString(Qt.locale(), 'f', v < 10 ? 1 : 0)) + " " + u[i]
    }

    function date(seconds) {
        if (!seconds)
            return ""
        return Qt.formatDateTime(new Date(seconds * 1000), italiano ? "dd/MM/yyyy HH:mm" : "yyyy-MM-dd HH:mm")
    }

    // rwxr-xr-x
    function modeString(mode) {
        var s = ""
        var c = "rwxrwxrwx"
        for (var i = 0; i < 9; ++i)
            s += (mode & (256 >> i)) ? c[i] : "-"
        return s
    }

    function count(n, one, many) { return n + " " + (n === 1 ? one : many) }
}
