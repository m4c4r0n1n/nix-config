// Recording stop button. Hidden from captures by an Umbriel window rule on its title.
import QtQuick
import Quickshell
import Quickshell.Io

FloatingWindow {
    id: win
    title: "shot-stop-button"
    implicitWidth: 176
    implicitHeight: 44
    color: "transparent"

    property string raw: ""
    property int secs: 0
    function pick(key, fallback) {
        var m = raw.match(new RegExp("^" + key + " *= *\"#([0-9a-fA-F]{6})", "m"));
        return m ? "#" + m[1] : fallback;
    }
    FileView {
        path: Quickshell.env("HOME") + "/.config/umbriel/noctalia.toml"
        blockLoading: true
        onLoaded: win.raw = text()
    }
    Timer { interval: 1000; running: true; repeat: true; onTriggered: win.secs++ }

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: mouse.containsMouse ? Qt.lighter(win.pick("background", "#1f1f1f"), 1.4) : win.pick("background", "#1f1f1f")
        border.width: 1
        border.color: win.pick("text_muted", "#c6c6c6")
        Row {
            anchors.centerIn: parent
            spacing: 10
            Rectangle {
                width: 12; height: 12; radius: 6; color: "#e23b3b"
                anchors.verticalCenter: parent.verticalCenter
                SequentialAnimation on opacity {
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.3; duration: 700 }
                    NumberAnimation { to: 1; duration: 700 }
                }
            }
            Text {
                text: Math.floor(win.secs / 60) + ":" + ("0" + win.secs % 60).slice(-2)
                color: win.pick("text_primary", "#e2e2e2")
                font.pixelSize: 15
                anchors.verticalCenter: parent.verticalCenter
            }
            Rectangle { width: 1; height: 20; color: win.pick("text_muted", "#c6c6c6"); anchors.verticalCenter: parent.verticalCenter }
            Text {
                text: "Stop"
                color: win.pick("text_primary", "#e2e2e2")
                font.pixelSize: 15; font.bold: true
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: Quickshell.execDetached([Quickshell.env("HOME") + "/.local/bin/shot-record", "stop"])
        }
    }
}
