// cham3 settings launcher - M1 foundation.
// Opens KDE System Settings (installed through cham3-desktop). That is the only
// action. The dedicated cham3 security sections of the planned Unified Control
// Center do not exist in M1 and are reported as unavailable; this launcher
// reads and changes no cham3 or security configuration.
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.plasma5support as P5Support
import org.kde.kirigami as Kirigami
import "../code/cham3_shared.js" as Shared

PlasmoidItem {
    id: root

    // Desktop entry shipped by the systemsettings package.
    readonly property string settings_entry: "systemsettings.desktop"

    toolTipMainText: "System Settings"
    toolTipSubText: "Opens KDE System Settings. cham3 security sections: " + Shared.state_unavailable + " (M1)."

    P5Support.DataSource {
        id: apps
        engine: "apps"
        function launch(entry) {
            var service = serviceForSource(entry)
            var operation = service.operationDescription("launch")
            service.startOperationCall(operation)
        }
    }

    // Always show the button; a click launches System Settings, never a popup.
    preferredRepresentation: compactRepresentation

    compactRepresentation: Item {
        id: compact
        Layout.fillWidth: true
        Layout.minimumHeight: 46
        Layout.preferredHeight: 46
        Layout.maximumHeight: 46

        Accessible.name: "Open System Settings. cham3 security sections unavailable."
        Accessible.role: Accessible.Button
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                apps.launch(root.settings_entry)
            }
        }

        Kirigami.Icon {
            anchors.centerIn: parent
            width: 32
            height: 32
            source: "preferences-system"
            active: mouse.containsMouse
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            radius: 5
            color: "transparent"
            border.width: compact.activeFocus ? 1 : 0
            border.color: Kirigami.Theme.textColor
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: apps.launch(root.settings_entry)
        }
    }
}
