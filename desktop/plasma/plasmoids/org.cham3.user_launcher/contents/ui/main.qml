// cham3 user launcher - M1 visual foundation.
// Shows a placeholder avatar and a flyout that (a) opens a fixed set of
// installed KDE applications and (b) offers the ordinary Lock / Log Out /
// Restart / Shut Down session actions. There is no identity service in M1:
// the flyout states "key_id: unavailable" and no avatar, name or key is shown
// as an authenticated cham3 identity. Nothing here enrolls, authenticates or
// validates anything. Categorized applications with contract badges and
// search are M5 scope and are not implied by this flyout.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.plasma5support as P5Support
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    // Applications installed by cham3-desktop dependencies. Desktop entry ids
    // verified against the Debian 13 packages (dolphin, konsole, kate, okular,
    // gwenview, kde-spectacle, ark, plasma-systemmonitor, systemsettings).
    readonly property var applications: [
        { label: "Files (Dolphin)", entry: "org.kde.dolphin.desktop", icon: "system-file-manager" },
        { label: "Terminal (Konsole)", entry: "org.kde.konsole.desktop", icon: "utilities-terminal" },
        { label: "Text editor (Kate)", entry: "org.kde.kate.desktop", icon: "kate" },
        { label: "Archives (Ark)", entry: "org.kde.ark.desktop", icon: "ark" },
        { label: "Documents (Okular)", entry: "org.kde.okular.desktop", icon: "okular" },
        { label: "Images (Gwenview)", entry: "org.kde.gwenview.desktop", icon: "gwenview" },
        { label: "Screenshots (Spectacle)", entry: "org.kde.spectacle.desktop", icon: "spectacle" },
        { label: "System Monitor", entry: "org.kde.plasma-systemmonitor.desktop", icon: "utilities-system-monitor" },
        { label: "System Settings", entry: "systemsettings.desktop", icon: "preferences-system" }
    ]

    toolTipMainText: "User launcher"
    toolTipSubText: "key_id: " + Shared.state_unavailable + " (no identity service present)"

    P5Support.DataSource {
        id: apps
        engine: "apps"
        function launch(entry) {
            var service = serviceForSource(entry)
            var operation = service.operationDescription("launch")
            service.startOperationCall(operation)
        }
    }

    Loader {
        id: session_loader
        source: Qt.resolvedUrl("session_actions.qml")
    }
    readonly property var session_item: session_loader.status === Loader.Ready ? session_loader.item : null

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 52
        Layout.preferredHeight: 52
        Layout.maximumHeight: 52

        Accessible.name: "User launcher. Opens applications and session actions. key_id unavailable."
        Accessible.role: Accessible.Button
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.expanded = !root.expanded
            }
        }

        // Circular placeholder avatar (generic glyph; not the user's identity).
        Rectangle {
            anchors.centerIn: parent
            width: 40
            height: 40
            radius: width / 2
            color: Shared.surface_raised
            border.width: 2
            border.color: compact.activeFocus ? Shared.text_primary : Shared.surface_border
            Kirigami.Icon {
                anchors.centerIn: parent
                width: 26
                height: 26
                source: "user-identity"
            }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: root.expanded = !root.expanded
        }
    }

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 22
        Layout.minimumHeight: Kirigami.Units.gridUnit * 20
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 3
            text: "cham3 user launcher"
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            font.bold: true
            wrapMode: Text.WordWrap
            text: "key_id: " + Shared.state_unavailable
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: "No cham3 identity service is present, so no identity or key is enrolled, authenticated or validated. The avatar is a generic placeholder."
        }

        Kirigami.Separator { Layout.fillWidth: true }

        PlasmaComponents.Label {
            text: "Applications"
            font.bold: true
        }
        GridLayout {
            Layout.fillWidth: true
            columns: 2
            Repeater {
                model: root.applications
                delegate: PlasmaComponents.Button {
                    required property int index
                    readonly property var application: root.applications[index]
                    Layout.fillWidth: true
                    text: application.label
                    icon.name: application.icon
                    onClicked: apps.launch(application.entry)
                }
            }
        }

        Kirigami.Separator { Layout.fillWidth: true }

        PlasmaComponents.Label {
            text: "Session"
            font.bold: true
        }
        RowLayout {
            Layout.fillWidth: true
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Lock"
                icon.name: "system-lock-screen"
                enabled: root.session_item !== null && root.session_item.can_lock
                onClicked: root.session_item.do_lock()
            }
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Log Out"
                icon.name: "system-log-out"
                enabled: root.session_item !== null && root.session_item.can_logout
                onClicked: root.session_item.do_logout()
            }
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Reboot"
                icon.name: "system-reboot"
                enabled: root.session_item !== null && root.session_item.can_reboot
                onClicked: root.session_item.do_reboot()
            }
            PlasmaComponents.Button {
                Layout.fillWidth: true
                text: "Power Off"
                icon.name: "system-shutdown"
                enabled: root.session_item !== null && root.session_item.can_shutdown
                onClicked: root.session_item.do_shutdown()
            }
        }
        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            visible: root.session_item === null
            text: "Session actions: " + Shared.state_unavailable + " (Plasma session module could not be loaded)."
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            text: "Search, categories and contract badges are not present (M5). Applications launched here are not contract-verified: " + Shared.state_unverified + ". " + Shared.foundation_notice
        }
    }
}
