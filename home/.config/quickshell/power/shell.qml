import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

ShellRoot {
    id: root

    property string raw: ""
    property int current: 0
    property real charge: 0
    property bool opened: false
    property real clockAngle: 0
    readonly property string dryFile: Quickshell.env("POWER_MENU_DRY") || ""
    readonly property real ringRadius: 215
    readonly property int orbSize: 112

    readonly property color paper: pick("background", "#1f1f1f")
    readonly property color ink: pick("text_primary", "#e2e2e2")
    readonly property color inkDim: pick("text_muted", "#c6c6c6")
    readonly property color accent: pick("accent_primary", "#ffffff")
    readonly property color accentTwo: pick("accent_secondary", "#c6c6c6")
    readonly property color alert: pick("error", "#ffb4ab")

    readonly property var actions: [
        { name: "lock", label: "Lock", glyph: "", hold: false },
        { name: "suspend", label: "Sleep", glyph: "", hold: false },
        { name: "logout", label: "Log Out", glyph: "", hold: true },
        { name: "reboot", label: "Reboot", glyph: "", hold: true },
        { name: "shutdown", label: "Shut Down", glyph: "", hold: true }
    ]

    function pick(key, fallback) {
        var m = raw.match(new RegExp("^" + key + " *= *\"#([0-9a-fA-F]{6})", "m"));
        return m ? "#" + m[1] : fallback;
    }

    function tone(index) {
        return actions[index].name === "shutdown" ? alert : (index % 2 === 0 ? accent : accentTwo);
    }

    function select(index) {
        if (index === current) {
            return;
        }
        release();
        current = (index + actions.length) % actions.length;
    }

    function run() {
        var name = actions[current].name;
        if (dryFile !== "") {
            Quickshell.execDetached(["sh", "-c", "echo " + name + " > '" + dryFile + "'"]);
        } else {
            Quickshell.execDetached(["noctalia", "msg", "session", name]);
        }
        Qt.quit();
    }

    function press() {
        if (actions[current].hold) {
            discharge.stop();
            charging.from = charge;
            charging.duration = 750 * (1 - charge);
            charging.start();
        } else {
            run();
        }
    }

    function release() {
        if (charging.running || charge > 0) {
            charging.stop();
            discharge.from = charge;
            discharge.start();
        }
    }

    NumberAnimation {
        id: charging
        target: root
        property: "charge"
        to: 1
        onFinished: if (root.charge >= 1) root.run()
    }

    NumberAnimation {
        id: discharge
        target: root
        property: "charge"
        to: 0
        duration: 220
        easing.type: Easing.OutCubic
    }

    FileView {
        path: Quickshell.env("HOME") + "/.config/umbriel/noctalia.toml"
        blockLoading: true
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.raw = text()
    }

    IpcHandler {
        target: "power"
        function select(index: int): void { root.select(index); }
        function press(): void { root.press(); }
        function release(): void { root.release(); }
        function close(): void { Qt.quit(); }
    }

    SystemClock {
        id: clock
        precision: SystemClock.Seconds
    }

    PanelWindow {
        id: window
        screen: Quickshell.screens[0]
        anchors { top: true; bottom: true; left: true; right: true }
        exclusionMode: ExclusionMode.Ignore
        color: "transparent"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
        WlrLayershell.namespace: "nana-power"

        Item {
            id: stage
            anchors.fill: parent
            focus: true
            Component.onCompleted: {
                forceActiveFocus();
                root.opened = true;
            }

            Keys.onPressed: event => {
                if (event.isAutoRepeat) {
                    event.accepted = true;
                    return;
                }
                var key = event.key;
                if (key === Qt.Key_Escape || key === Qt.Key_Q) {
                    Qt.quit();
                } else if (key === Qt.Key_Left || key === Qt.Key_H || key === Qt.Key_K || key === Qt.Key_Up) {
                    root.select(root.current - 1);
                } else if (key === Qt.Key_Right || key === Qt.Key_L || key === Qt.Key_J || key === Qt.Key_Down || key === Qt.Key_Tab) {
                    root.select(root.current + 1);
                } else if (key >= Qt.Key_1 && key <= Qt.Key_5) {
                    root.select(key - Qt.Key_1);
                } else if (key === Qt.Key_Return || key === Qt.Key_Enter || key === Qt.Key_Space) {
                    root.press();
                }
                event.accepted = true;
            }
            Keys.onReleased: event => {
                if (!event.isAutoRepeat && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space)) {
                    root.release();
                }
            }

            Rectangle {
                anchors.fill: parent
                color: root.paper
                opacity: root.opened ? 0.58 : 0
                Behavior on opacity { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
            }

            MouseArea {
                anchors.fill: parent
                onClicked: Qt.quit()
            }

            Item {
                id: orbit
                anchors.centerIn: parent
                width: 2 * root.ringRadius + root.orbSize + 80
                height: width
                scale: root.opened ? 1 : 0.72
                opacity: root.opened ? 1 : 0
                Behavior on scale { NumberAnimation { duration: 420; easing.type: Easing.OutBack; easing.overshoot: 1.4 } }
                Behavior on opacity { NumberAnimation { duration: 240 } }

                Rectangle {
                    anchors.centerIn: parent
                    width: 2 * root.ringRadius
                    height: width
                    radius: width / 2
                    color: "transparent"
                    border.width: 1
                    border.color: Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.16)
                }

                Item {
                    id: spinner
                    anchors.fill: parent
                    RotationAnimation on rotation {
                        from: 0
                        to: 360
                        duration: 48000
                        loops: Animation.Infinite
                    }
                    Repeater {
                        model: 3
                        Rectangle {
                            required property int index
                            width: 6
                            height: 6
                            radius: 3
                            color: root.accent
                            opacity: 0.8
                            x: spinner.width / 2 + (root.ringRadius - 74) * Math.cos(2 * Math.PI * index / 3) - 3
                            y: spinner.height / 2 + (root.ringRadius - 74) * Math.sin(2 * Math.PI * index / 3) - 3
                        }
                    }
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 6
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Qt.formatTime(clock.date, "hh:mm")
                        color: root.ink
                        font.family: "Maple Mono NF"
                        font.pixelSize: 78
                        font.weight: Font.Bold
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Qt.formatDate(clock.date, "dddd, MMMM d")
                        color: root.inkDim
                        font.family: "Maple Mono NF"
                        font.pixelSize: 19
                    }
                    Item { width: 1; height: 14 }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.actions[root.current].label
                        color: root.tone(root.current)
                        font.family: "Maple Mono NF"
                        font.pixelSize: 26
                        font.weight: Font.DemiBold
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.actions[root.current].hold ? "hold to confirm" : "click to confirm"
                        color: root.inkDim
                        opacity: 0.7
                        font.family: "Maple Mono NF"
                        font.pixelSize: 14
                    }
                }

                Repeater {
                    model: root.actions.length
                    Item {
                        id: orb
                        required property int index
                        readonly property real angle: -Math.PI / 2 + 2 * Math.PI * index / root.actions.length
                        readonly property bool active: index === root.current
                        readonly property color tint: root.tone(index)
                        width: root.orbSize
                        height: root.orbSize
                        x: orbit.width / 2 + root.ringRadius * Math.cos(angle) - width / 2
                        y: orbit.height / 2 + root.ringRadius * Math.sin(angle) - height / 2
                        scale: active ? 1.16 : 1
                        Behavior on scale { NumberAnimation { duration: 170; easing.type: Easing.OutBack } }

                        Rectangle {
                            anchors.centerIn: parent
                            width: parent.width + 26
                            height: width
                            radius: width / 2
                            color: orb.tint
                            opacity: orb.active ? 0.16 + 0.22 * root.charge : 0
                            Behavior on opacity { NumberAnimation { duration: 170 } }
                        }

                        Rectangle {
                            anchors.fill: parent
                            radius: width / 2
                            color: Qt.rgba(root.paper.r, root.paper.g, root.paper.b, 0.72)
                            border.width: orb.active ? 2 : 1
                            border.color: orb.active ? orb.tint : Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.28)
                        }

                        Canvas {
                            id: arc
                            anchors.fill: parent
                            visible: orb.active && root.charge > 0
                            property real value: orb.active ? root.charge : 0
                            onValueChanged: requestPaint()
                            onPaint: {
                                var c = getContext("2d");
                                c.reset();
                                c.lineWidth = 5;
                                c.lineCap = "round";
                                c.strokeStyle = orb.tint;
                                c.beginPath();
                                c.arc(width / 2, height / 2, width / 2 - 4, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * value);
                                c.stroke();
                            }
                        }

                        Text {
                            anchors.centerIn: parent
                            text: root.actions[orb.index].glyph
                            color: orb.active ? orb.tint : root.ink
                            font.family: "Maple Mono NF"
                            font.pixelSize: 38
                        }

                        Text {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.margins: 4
                            text: orb.index + 1
                            color: root.inkDim
                            opacity: 0.6
                            font.family: "Maple Mono NF"
                            font.pixelSize: 12
                        }

                        MouseArea {
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onPositionChanged: if (root.dryFile === "") root.select(orb.index)
                            onPressed: {
                                root.select(orb.index);
                                root.press();
                            }
                            onReleased: root.release()
                            onCanceled: root.release()
                        }
                    }
                }
            }
        }
    }
}
