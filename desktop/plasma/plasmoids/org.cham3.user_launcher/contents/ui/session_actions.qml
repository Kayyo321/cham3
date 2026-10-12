// Ordinary desktop session actions for the cham3 user launcher.
// Loaded through a Loader so that, if the Plasma-private sessions module is
// missing or changes, only the session buttons degrade (they stay disabled) and
// the rest of the launcher keeps working. These are standard KDE Lock / Log Out
// / Restart / Shut Down requests; KDE shows its own confirmation. They are not
// cham3 backends and do not touch identity, keys, contracts or networking.
// org.kde.plasma.private.sessions is a Plasma-private module (external
// boundary); it is pinned to the Debian 13 Plasma 6.3 series.
import QtQuick
import org.kde.plasma.private.sessions

Item {
    id: root

    readonly property bool can_lock: session.canLock
    readonly property bool can_logout: session.canLogout
    readonly property bool can_reboot: session.canReboot
    readonly property bool can_shutdown: session.canShutdown

    function do_lock() { session.lock() }
    function do_logout() { session.requestLogout() }
    function do_reboot() { session.requestReboot() }
    function do_shutdown() { session.requestShutdown() }

    SessionManagement {
        id: session
    }
}
