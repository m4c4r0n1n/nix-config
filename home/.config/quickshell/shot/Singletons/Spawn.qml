pragma Singleton
import Quickshell
Singleton {
    readonly property var env: ({})
    function run(argv) { Quickshell.execDetached(argv); }
    function runApp(argv, opts) { Quickshell.execDetached(argv); }
}
