import QtQuick
import "."

// Un avviso breve in basso, che sparisce da solo.
SteelPanel {
    id: toast
    led: ""
    overlay: true
    property bool error: false
    function show(text, isError) {
        label.text = text
        error = !!isError
        opacity = 1
        timer.restart()
    }
    anchors.horizontalCenter: parent.horizontalCenter
    width: Math.min(parent.width - 2 * Style.margin, label.implicitWidth + 2 * Style.margin)
    height: label.implicitHeight + 20 * Style.dp
    radius: 10 * Style.dp
    border.color: error ? Style.danger : Style.accent
    border.width: 1.5 * Style.dp
    opacity: 0
    visible: opacity > 0
    Behavior on opacity { NumberAnimation { duration: 180 } }
    Text {
        font.family: Style.fontFamily
        id: label
        anchors.centerIn: parent
        width: Math.min(implicitWidth, toast.parent.width - 4 * Style.margin)
        wrapMode: Text.Wrap
        color: Style.text
        font.pixelSize: Style.fontM
    }
    Timer { id: timer; interval: toast.error ? 5000 : 2500; onTriggered: toast.opacity = 0 }
    MouseArea { anchors.fill: parent; onClicked: toast.opacity = 0 }
}
