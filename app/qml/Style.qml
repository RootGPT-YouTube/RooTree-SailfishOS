pragma Singleton
import QtQuick

// Colori, misure e testi comuni. `dp` lo imposta Main.qml dalla larghezza
// della finestra: 1 dp = 1/400 del lato corto (2,7 px sul POCO).
QtObject {
    property real dp: 2.7

    readonly property bool italiano: Qt.locale().name.substring(0, 2) === "it"
    function t(it, en) { return italiano ? it : en }

    // il tema: "rootundra" (acciaio, LED blu, ghiaccio) o "sailfish" (piatto,
    // con i colori e lo sfondo dell'ambience attiva); lo imposta Main.qml
    property string theme: "rootundra"
    readonly property bool sailfish: theme === "sailfish"
    // l'ambience letta da Fs.ambience() (Main.qml la rilegge al cambio di tema)
    property var amb: ({})
    readonly property bool lightScheme: sailfish && amb.lightScheme === true
    readonly property color sfHighlight: amb.highlight || "#ff80f3ff"
    readonly property color sfPrimary: amb.primary || (lightScheme ? "#ff000000" : "#ffffffff")
    readonly property color sfSecondary: amb.secondary || (lightScheme ? "#99000000" : "#b3ffffff")
    readonly property color sfSecondaryHighlight: amb.secondaryHighlight || Qt.darker(sfHighlight, 1.3)
    function alpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }

    // i colori di RooTundra (tastierino della schermata di blocco): acciaio
    // scuro, testo bianco ghiaccio, secondario azzurro ghiaccio, LED blu elettrico
    readonly property color bg: sailfish ? (lightScheme ? "#ffffff" : "#000000") : "#14181d"
    readonly property color bg2: sailfish ? bg : "#20262d"
    readonly property color panel: sailfish ? alpha(sfHighlight, 0.12) : "#2a3139"
    readonly property color panel2: sailfish ? alpha(sfHighlight, 0.2) : "#3a434d"
    readonly property color line: sailfish ? alpha(sfPrimary, 0.2) : "#3a434d"
    readonly property color steelBorder: sailfish ? alpha(sfPrimary, 0.4) : "#6b7b8c"
    readonly property color text: sailfish ? sfPrimary : "#eef6ff"
    readonly property color dim: sailfish ? sfSecondary : "#a9c4e0"
    readonly property color accent: sailfish ? sfHighlight : "#1a7dff"
    readonly property color accentSoft: sailfish ? alpha(sfHighlight, 0.25) : "#401a7dff"
    readonly property color selected: sailfish ? alpha(sfHighlight, 0.2) : "#331a7dff"
    readonly property color pressed: sailfish ? alpha(sfHighlight, 0.25) : "#14ffffff"
    readonly property color danger: "#ff5a5a"
    readonly property color ok: "#5ccf8a"
    // Sailfish: le barre sono velate del colore dell'ambience, i pannelli che si aprono
    // sopra sono scuri (o chiari) con una velatura del colore dell'ambience
    readonly property color sfBar: alpha(sfHighlight, 0.1)
    readonly property color sfOverlay: Qt.tint(lightScheme ? "#f2ffffff" : "#f20a0c10", alpha(sfHighlight, 0.12))
    // le icone (già colorate per RooTundra) si ricolorano in Sailfish
    readonly property color iconTint: sailfish ? sfPrimary : "transparent"

    // Rajdhani è un carattere stretto: misure un po' più grandi del solito;
    // Sail Sans Pro è più largo, quindi lo si rimpicciolisce un poco
    readonly property string fontFamily: sailfish && amb.font ? amb.font : "Rajdhani"
    readonly property real fontScale: sailfish ? 0.9 : 1
    readonly property real fontS: 13.5 * dp * fontScale
    readonly property real fontM: 16 * dp * fontScale
    readonly property real fontL: 19.5 * dp * fontScale
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
