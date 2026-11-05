pragma Singleton
import Quickshell
Singleton {
    function tr(s) {
        var out = String(s);
        for (var i = 1; i < arguments.length; i++) out = out.replace("%" + i, arguments[i]);
        return out;
    }
}
