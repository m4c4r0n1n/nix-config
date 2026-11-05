pragma Singleton
// Palette comes from the file Noctalia regenerates on every theme change.
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root
    property string raw: ""
    function pick(key, fallback) {
        var m = raw.match(new RegExp("^" + key + " *= *\"#([0-9a-fA-F]{6})", "m"));
        return m ? "#" + m[1] : fallback;
    }
    readonly property color paper: pick("background", "#1f1f1f")
    readonly property color paperLift: Qt.lighter(paper, 1.25)
    readonly property color ink: pick("text_primary", "#e2e2e2")
    readonly property color inkDim: pick("text_muted", "#c6c6c6")
    readonly property color inkFaint: Qt.rgba(ink.r, ink.g, ink.b, 0.45)
    readonly property color sun: pick("accent_primary", "#ffffff")
    readonly property color alert: pick("error", "#ffb4ab")
    readonly property color lineSoft: Qt.rgba(ink.r, ink.g, ink.b, 0.14)
    readonly property color tint5: Qt.rgba(ink.r, ink.g, ink.b, 0.05)
    readonly property color tint10: Qt.rgba(ink.r, ink.g, ink.b, 0.10)
    readonly property color tint16: Qt.rgba(ink.r, ink.g, ink.b, 0.16)
    readonly property string ui: "sans-serif"
    readonly property string mono: "Maple Mono NF"
    readonly property string jp: "sans-serif"
    readonly property int radius: 6
    readonly property int snap: 120
    readonly property int move: 200
    FileView {
        path: Quickshell.env("HOME") + "/.config/umbriel/noctalia.toml"
        blockLoading: true
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.raw = text()
    }
}
